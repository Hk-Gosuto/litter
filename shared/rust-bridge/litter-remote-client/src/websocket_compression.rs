//! Codex zstd-stream-v1: separate persistent streams, flushed once per JSON-RPC message.
//! Adapted from upstream #57218 without the server/execution dependencies.

use std::io;
use zstd::stream::raw::{CParameter, DParameter, Decoder, Encoder, Operation, OutBuffer};

pub(crate) const HEADER: &str = "x-codex-app-server-compression";
pub(crate) const PROTOCOL: &str = "zstd-stream-v1";
const BUFFER_SIZE: usize = 32 * 1024;

pub(crate) struct Codec {
    encoder: Encoder<'static>,
    decoder: Decoder<'static>,
    max_message_size: usize,
}

impl Codec {
    pub(crate) fn negotiate(
        value: Option<&str>,
        max_message_size: usize,
    ) -> io::Result<Option<Self>> {
        match value {
            None => Ok(None),
            Some(PROTOCOL) => {
                let mut encoder = Encoder::new(-7)?;
                encoder.set_parameter(CParameter::WindowLog(20))?;
                let mut decoder = Decoder::new()?;
                decoder.set_parameter(DParameter::WindowLogMax(20))?;
                Ok(Some(Self {
                    encoder,
                    decoder,
                    max_message_size,
                }))
            }
            Some(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported app-server WebSocket compression protocol",
            )),
        }
    }

    pub(crate) fn encode(&mut self, text: &str) -> io::Result<Vec<u8>> {
        let mut input = text.as_bytes();
        let mut encoded = Vec::new();
        let mut buffer = [0; BUFFER_SIZE];
        while !input.is_empty() {
            let status = self.encoder.run_on_buffers(input, &mut buffer)?;
            encoded.extend_from_slice(&buffer[..status.bytes_written]);
            input = &input[status.bytes_read..];
        }
        loop {
            let mut output = OutBuffer::around(&mut buffer[..]);
            let remaining = self.encoder.flush(&mut output)?;
            encoded.extend_from_slice(output.as_slice());
            if remaining == 0 {
                return Ok(encoded);
            }
        }
    }

    pub(crate) fn decode(&mut self, mut input: &[u8]) -> io::Result<String> {
        let mut decoded = Vec::new();
        let mut buffer = [0; BUFFER_SIZE];
        loop {
            let status = self.decoder.run_on_buffers(input, &mut buffer)?;
            if status.bytes_written > self.max_message_size.saturating_sub(decoded.len()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "decompressed app-server message exceeds size limit",
                ));
            }
            decoded.extend_from_slice(&buffer[..status.bytes_written]);
            input = &input[status.bytes_read..];
            if status.bytes_read == 0 && status.bytes_written == 0 {
                if !input.is_empty() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid compressed app-server message",
                    ));
                }
                break;
            }
        }
        String::from_utf8(decoded).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec_retains_history_and_reconnect_starts_fresh() {
        let mut sender = Codec::negotiate(Some(PROTOCOL), 100_000).unwrap().unwrap();
        let mut receiver = Codec::negotiate(Some(PROTOCOL), 100_000).unwrap().unwrap();
        let message = "你好 app-server history ".repeat(1000);
        let first = sender.encode(&message).unwrap();
        assert_eq!(receiver.decode(&first).unwrap(), message);
        let second = sender.encode(&message).unwrap();
        assert_eq!(receiver.decode(&second).unwrap(), message);
        assert!(second.len() < first.len());
        let mut fresh = Codec::negotiate(Some(PROTOCOL), 100_000).unwrap().unwrap();
        assert_eq!(fresh.decode(&first).unwrap(), message);
    }

    #[test]
    fn codec_negotiation_is_optional_and_decoded_sizes_are_bounded() {
        assert!(Codec::negotiate(None, 1).unwrap().is_none());
        assert!(Codec::negotiate(Some("unknown"), 1).is_err());
        let mut sender = Codec::negotiate(Some(PROTOCOL), 100_000).unwrap().unwrap();
        let mut receiver = Codec::negotiate(Some(PROTOCOL), 10).unwrap().unwrap();
        assert!(
            receiver
                .decode(&sender.encode(&"x".repeat(1000)).unwrap())
                .is_err()
        );
    }
}

//! SSH session bring-up and teardown: TCP dial → russh handshake →
//! host-key verification (via the user's `host_key_callback`) →
//! authenticate → ready-to-use [`SshClient`].

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::BoxFuture;
use russh::client;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg, PublicKey, decode_secret_key};
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use super::{
    CONNECT_TIMEOUT, KEEPALIVE_INTERVAL, SSH_CHANNEL_BUFFER_SIZE, SSH_CHANNEL_WINDOW_SIZE,
    SSH_MAX_PACKET_SIZE, SshAuth, SshClient, SshCredentials, SshDetection, SshError,
    append_bridge_info_log, normalize_host,
};

pub(super) type HostKeyCallback = Arc<dyn Fn(&str) -> BoxFuture<'static, bool> + Send + Sync>;

pub(super) struct ClientHandler {
    pub(super) host_key_cb: HostKeyCallback,
    /// If the callback rejects the key we store the fingerprint so we can
    /// surface it in [`SshError::HostKeyVerification`].
    pub(super) rejected_fingerprint: Arc<Mutex<Option<String>>>,
    pub(super) verified_fingerprint: Arc<Mutex<Option<String>>>,
}

#[async_trait]
impl client::Handler for ClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        let fp = format!("{}", server_public_key.fingerprint(HashAlg::Sha256));
        let rejected_fingerprint = Arc::clone(&self.rejected_fingerprint);
        let callback = Arc::clone(&self.host_key_cb);
        let verified_fingerprint = Arc::clone(&self.verified_fingerprint);
        async move {
            let accepted = callback(&fp).await;
            if accepted {
                *verified_fingerprint.lock().await = Some(fp);
            } else {
                *rejected_fingerprint.lock().await = Some(fp);
            }
            Ok(accepted)
        }
    }
}

impl SshClient {
    /// Open an SSH connection to `credentials.host:credentials.port`.
    ///
    /// `host_key_callback` is invoked with the SHA-256 fingerprint of the
    /// server's public key. Return `true` to accept, `false` to reject.
    pub async fn connect(
        credentials: SshCredentials,
        host_key_callback: Box<dyn Fn(&str) -> BoxFuture<'static, bool> + Send + Sync>,
    ) -> Result<Self, SshError> {
        let macos_keychain_password = if credentials.unlock_macos_keychain {
            match &credentials.auth {
                SshAuth::Password(password) => Some(password.clone()),
                SshAuth::PrivateKey { .. } => None,
            }
        } else {
            None
        };
        let auth_kind = match &credentials.auth {
            SshAuth::Password(_) => "password",
            SshAuth::PrivateKey { .. } => "key",
        };
        let rejected_fp = Arc::new(Mutex::new(None));
        let verified_fp = Arc::new(Mutex::new(None));

        let handler = ClientHandler {
            host_key_cb: Arc::from(host_key_callback),
            rejected_fingerprint: Arc::clone(&rejected_fp),
            verified_fingerprint: Arc::clone(&verified_fp),
        };

        let config = client::Config {
            keepalive_interval: Some(KEEPALIVE_INTERVAL),
            keepalive_max: 3,
            inactivity_timeout: None,
            window_size: SSH_CHANNEL_WINDOW_SIZE,
            maximum_packet_size: SSH_MAX_PACKET_SIZE,
            channel_buffer_size: SSH_CHANNEL_BUFFER_SIZE,
            nodelay: true,
            ..Default::default()
        };

        let addr = format!("{}:{}", normalize_host(&credentials.host), credentials.port);
        info!(
            "SSH connect start addr={} username={} auth={} nodelay={} window_size={} maximum_packet_size={} channel_buffer_size={}",
            addr,
            credentials.username,
            auth_kind,
            config.nodelay,
            config.window_size,
            config.maximum_packet_size,
            config.channel_buffer_size
        );
        append_bridge_info_log(&format!(
            "ssh_connect_start addr={} username={} auth={} nodelay={} window_size={} maximum_packet_size={} channel_buffer_size={}",
            addr,
            credentials.username,
            auth_kind,
            config.nodelay,
            config.window_size,
            config.maximum_packet_size,
            config.channel_buffer_size
        ));

        let connect_result = tokio::time::timeout(
            CONNECT_TIMEOUT,
            client::connect(Arc::new(config), &*addr, handler),
        )
        .await;
        let mut handle = match connect_result {
            Ok(Ok(handle)) => handle,
            Ok(Err(error)) => {
                // russh surfaces a rejected host key as `UnknownKey` from
                // `client::connect`; check the rejected-fingerprint flag
                // before falling back to a generic connect error so callers
                // can distinguish a changed host key from an unknown one.
                if let Some(fp) = rejected_fp.lock().await.take() {
                    warn!("SSH host key rejected addr={} fingerprint={}", addr, fp);
                    append_bridge_info_log(&format!(
                        "ssh_host_key_rejected addr={} fingerprint={}",
                        addr, fp
                    ));
                    return Err(SshError::HostKeyVerification { fingerprint: fp });
                }
                error!("SSH connect failed addr={} error={:?}", addr, error);
                append_bridge_info_log(&format!(
                    "ssh_connect_failed addr={} error_display={} error_debug={:?}",
                    addr, error, error
                ));
                return Err(SshError::ConnectionFailed(format!("{error}")));
            }
            Err(_) => {
                warn!("SSH connect timed out addr={}", addr);
                append_bridge_info_log(&format!("ssh_connect_timeout addr={}", addr));
                return Err(SshError::Timeout);
            }
        };

        // If the handler rejected the key, surface a specific error.
        if let Some(fp) = rejected_fp.lock().await.take() {
            warn!("SSH host key rejected addr={} fingerprint={}", addr, fp);
            append_bridge_info_log(&format!(
                "ssh_host_key_rejected addr={} fingerprint={}",
                addr, fp
            ));
            return Err(SshError::HostKeyVerification { fingerprint: fp });
        }

        let auth_result = match &credentials.auth {
            SshAuth::Password(pw) => handle
                .authenticate_password(&credentials.username, pw)
                .await
                .map_err(|e| {
                    warn!("SSH password auth failed addr={} error={:?}", addr, e);
                    append_bridge_info_log(&format!(
                        "ssh_auth_failed addr={} method=password error_display={} error_debug={:?}",
                        addr, e, e
                    ));
                    SshError::AuthFailed(format!("{e}"))
                })?,
            SshAuth::PrivateKey {
                key_pem,
                passphrase,
            } => {
                let key = decode_secret_key(key_pem, passphrase.as_deref())
                    .map_err(|e| SshError::AuthFailed(format!("bad private key: {e}")))?;
                let key = PrivateKeyWithHashAlg::new(
                    Arc::new(key),
                    handle.best_supported_rsa_hash().await.map_err(|e| {
                        warn!("SSH RSA hash negotiation failed addr={} error={:?}", addr, e);
                        append_bridge_info_log(&format!(
                            "ssh_auth_failed addr={} method=key_hash error_display={} error_debug={:?}",
                            addr, e, e
                        ));
                        SshError::AuthFailed(format!("{e}"))
                    })?
                    .flatten(),
                );
                handle
                    .authenticate_publickey(&credentials.username, key)
                    .await
                    .map_err(|e| {
                        warn!("SSH key auth failed addr={} error={:?}", addr, e);
                        append_bridge_info_log(&format!(
                            "ssh_auth_failed addr={} method=key error_display={} error_debug={:?}",
                            addr, e, e
                        ));
                        SshError::AuthFailed(format!("{e}"))
                    })?
            }
        };

        if !auth_result.success() {
            warn!(
                "SSH auth rejected by server addr={} username={}",
                addr, credentials.username
            );
            append_bridge_info_log(&format!(
                "ssh_auth_rejected addr={} username={}",
                addr, credentials.username
            ));
            return Err(SshError::AuthFailed("server rejected credentials".into()));
        }

        info!("SSH connected and authenticated to {addr}");
        append_bridge_info_log(&format!(
            "ssh_connect_success addr={} username={}",
            addr, credentials.username
        ));

        let verified_fingerprint = verified_fp.lock().await.clone().ok_or_else(|| {
            SshError::ConnectionFailed("SSH handshake did not verify a host key".into())
        })?;
        Ok(Self {
            handle: Arc::new(Mutex::new(handle)),
            credentials,
            verified_fingerprint,
            reconnect_lock: Mutex::new(()),
            disconnected: std::sync::atomic::AtomicBool::new(false),
            forward_tasks: Mutex::new(HashMap::new()),
            macos_keychain_password,
            detection: std::sync::Mutex::new(SshDetection::default()),
            shell_probe: Mutex::new(()),
        })
    }

    /// Re-authenticate a dropped SSH transport before rebuilding its RPC stream.
    /// Existing port listeners share `handle`, so they use the replacement too.
    pub(crate) async fn reconnect_if_needed(&self) -> Result<(), SshError> {
        use std::sync::atomic::Ordering;
        let _guard = self.reconnect_lock.lock().await;
        if self.disconnected.load(Ordering::SeqCst) {
            return Err(SshError::ConnectionFailed(
                "SSH session was explicitly disconnected".into(),
            ));
        }
        if !self.handle.lock().await.is_closed() {
            return Ok(());
        }
        let expected = self.verified_fingerprint.clone();
        let next = tokio::time::timeout(
            CONNECT_TIMEOUT * 3,
            Self::connect(
                self.credentials.clone(),
                Box::new(move |fingerprint| {
                    // Pin the key actually verified on the first connection, even
                    // if the original callback allowed trusting an unknown host.
                    let accepted = fingerprint == expected;
                    Box::pin(async move { accepted })
                }),
            ),
        )
        .await
        .map_err(|_| SshError::Timeout)??;
        let mut handle = self.handle.lock().await;
        if self.disconnected.load(Ordering::SeqCst) {
            drop(handle);
            next.disconnect().await;
            return Err(SshError::ConnectionFailed(
                "SSH session was explicitly disconnected".into(),
            ));
        }
        std::mem::swap(&mut *handle, &mut *next.handle.lock().await);
        Ok(())
    }

    /// Whether the SSH session appears to still be connected.
    pub fn is_connected(&self) -> bool {
        match self.handle.try_lock() {
            Ok(h) => !h.is_closed(),
            Err(_) => true, // locked = in use = presumably connected
        }
    }

    /// Disconnect the SSH session, aborting any port forwards.
    pub async fn disconnect(&self) {
        self.disconnected
            .store(true, std::sync::atomic::Ordering::SeqCst);
        // Abort all forwarding tasks.
        let mut tasks = self.forward_tasks.lock().await;
        for (_, task) in tasks.drain() {
            task.task.abort();
        }
        drop(tasks);

        let handle = self.handle.lock().await;
        let _ = handle
            .disconnect(russh::Disconnect::ByApplication, "bye", "en")
            .await;
    }
}

#[cfg(test)]
mod reconnect_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;

    struct TestServer {
        authentications: Arc<AtomicUsize>,
    }

    impl russh::server::Handler for TestServer {
        type Error = russh::Error;

        async fn auth_password(
            &mut self,
            user: &str,
            password: &str,
        ) -> Result<russh::server::Auth, Self::Error> {
            assert_eq!((user, password), ("test-user", "test-password"));
            self.authentications.fetch_add(1, Ordering::SeqCst);
            Ok(russh::server::Auth::Accept)
        }

        async fn channel_open_session(
            &mut self,
            _channel: russh::Channel<russh::server::Msg>,
            reply: russh::server::ChannelOpenHandle,
            _session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }

        async fn exec_request(
            &mut self,
            channel: russh::ChannelId,
            _command: &[u8],
            session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            session.channel_success(channel)?;
            session.data(channel, b"reconnected SSH channel".to_vec())?;
            session.exit_status_request(channel, 0)?;
            session.close(channel)?;
            Ok(())
        }

        async fn channel_open_direct_tcpip(
            &mut self,
            _channel: russh::Channel<russh::server::Msg>,
            _host: &str,
            _port: u32,
            _origin: &str,
            _origin_port: u32,
            reply: russh::server::ChannelOpenHandle,
            _session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            reply.accept().await;
            Ok(())
        }

        async fn data(
            &mut self,
            channel: russh::ChannelId,
            bytes: &[u8],
            session: &mut russh::server::Session,
        ) -> Result<(), Self::Error> {
            session.data(channel, bytes.to_vec())?;
            Ok(())
        }
    }

    async fn test_server(
        rotate_key: bool,
    ) -> (
        SshCredentials,
        mpsc::Receiver<russh::server::Handle>,
        Arc<AtomicUsize>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let (handles_tx, handles_rx) = mpsc::channel(4);
        let authentications = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&authentications);
        let task = tokio::spawn(async move {
            for index in 0..2 {
                let seed = if rotate_key && index == 1 { 43 } else { 42 };
                let key = russh::keys::ssh_key::private::Ed25519Keypair::from_seed(&[seed; 32]);
                let config = Arc::new(russh::server::Config {
                    keys: vec![key.into()],
                    auth_rejection_time: std::time::Duration::ZERO,
                    ..Default::default()
                });
                let (stream, _) = listener.accept().await.unwrap();
                let session = russh::server::run_stream(
                    config,
                    stream,
                    TestServer {
                        authentications: Arc::clone(&count),
                    },
                )
                .await
                .unwrap();
                handles_tx.send(session.handle()).await.unwrap();
                tokio::spawn(async move {
                    let _ = session.await;
                });
            }
        });
        (
            SshCredentials {
                host: "127.0.0.1".into(),
                port,
                username: "test-user".into(),
                auth: SshAuth::Password("test-password".into()),
                unlock_macos_keychain: false,
            },
            handles_rx,
            authentications,
            task,
        )
    }

    async fn drop_server_connection(
        client: &SshClient,
        handles: &mut mpsc::Receiver<russh::server::Handle>,
    ) {
        handles
            .recv()
            .await
            .unwrap()
            .disconnect(
                russh::Disconnect::ByApplication,
                "simulate SSH outage".into(),
                "en".into(),
            )
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while client.is_connected() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("SSH closure should be observed");
    }

    #[tokio::test]
    async fn dropped_ssh_reauthenticates_once_and_opens_new_channels() {
        let (credentials, mut handles, authentications, server) = test_server(false).await;
        let client = SshClient::connect(credentials, Box::new(|_| Box::pin(async { true })))
            .await
            .unwrap();
        client.reconnect_if_needed().await.unwrap();
        assert_eq!(authentications.load(Ordering::SeqCst), 1);
        let port = client.forward_port_to(0, "test-host", 8390).await.unwrap();
        assert_forwarding_works(port).await;
        drop_server_connection(&client, &mut handles).await;
        let (first, second) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            tokio::join!(client.reconnect_if_needed(), client.reconnect_if_needed())
        })
        .await
        .unwrap();
        first.unwrap();
        second.unwrap();
        assert_eq!(authentications.load(Ordering::SeqCst), 2);
        // The existing local listener must pick up the replacement handle;
        // no rebind or navigation is needed for the WebSocket tunnel.
        assert_forwarding_works(port).await;
        assert_eq!(
            client.exec("test").await.unwrap().stdout,
            "reconnected SSH channel"
        );
        client.disconnect().await;
        assert!(
            client.reconnect_if_needed().await.is_err(),
            "manual disconnect must not revive SSH"
        );
        server.abort();
    }

    async fn assert_forwarding_works(port: u16) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
                .await
                .unwrap();
            stream.write_all(b"ping").await.unwrap();
            let mut reply = [0; 4];
            stream.read_exact(&mut reply).await.unwrap();
            assert_eq!(&reply, b"ping");
        })
        .await
        .expect("forwarded channel should use the live SSH handle");
    }

    #[tokio::test]
    async fn reconnect_rejects_changed_host_key_even_after_trusting_first_connection() {
        let (credentials, mut handles, authentications, server) = test_server(true).await;
        let client = SshClient::connect(credentials, Box::new(|_| Box::pin(async { true })))
            .await
            .unwrap();
        drop_server_connection(&client, &mut handles).await;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.reconnect_if_needed(),
        )
        .await
        .unwrap();
        assert!(matches!(result, Err(SshError::HostKeyVerification { .. })));
        assert_eq!(authentications.load(Ordering::SeqCst), 1);
        client.disconnect().await;
        server.abort();
    }
}

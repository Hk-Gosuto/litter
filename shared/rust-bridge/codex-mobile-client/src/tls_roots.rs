use std::path::Path;

/// Remote WSS/HTTPS connections still need CA roots after removing local startup.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub(crate) fn init_mobile_tls_roots() {
    if std::env::var_os("SSL_CERT_FILE")
        .is_some_and(|path| is_usable_pem_bundle(Path::new(&path)))
    {
        return;
    }
    let directory = std::env::temp_dir().join("litter-tls");
    let bundle = directory.join("cacert.pem");
    let result = std::fs::create_dir_all(&directory).and_then(|_| {
        if is_usable_pem_bundle(&bundle) {
            Ok(())
        } else {
            std::fs::write(&bundle, include_bytes!("cacert.pem"))
        }
    });
    match result {
        Ok(()) => unsafe { std::env::set_var("SSL_CERT_FILE", bundle) },
        Err(error) => tracing::warn!("failed to prepare mobile TLS roots: {error}"),
    }
}

pub(crate) fn is_usable_pem_bundle(path: &Path) -> bool {
    const CERTIFICATE_HEADER: &[u8] = b"-----BEGIN CERTIFICATE-----";
    const CERTIFICATE_FOOTER: &[u8] = b"-----END CERTIFICATE-----";
    std::fs::read(path)
        .map(|contents| {
            let Some(header_index) = contents
                .windows(CERTIFICATE_HEADER.len())
                .position(|window| window == CERTIFICATE_HEADER)
            else {
                return false;
            };
            let certificate_body = &contents[header_index + CERTIFICATE_HEADER.len()..];
            let Some(footer_index) = certificate_body
                .windows(CERTIFICATE_FOOTER.len())
                .position(|window| window == CERTIFICATE_FOOTER)
            else {
                return false;
            };
            certificate_body[..footer_index]
                .iter()
                .any(|byte| !byte.is_ascii_whitespace())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::is_usable_pem_bundle;

    #[test]
    fn pem_bundle_validation_rejects_empty_or_truncated_files() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let pem_path = temp_dir.path().join("cacert.pem");

        std::fs::write(&pem_path, []).expect("write empty bundle");
        assert!(!is_usable_pem_bundle(&pem_path));

        std::fs::write(&pem_path, b"-----BEGIN CERT").expect("write truncated bundle");
        assert!(!is_usable_pem_bundle(&pem_path));

        std::fs::write(&pem_path, b"-----BEGIN CERTIFICATE-----\nZm9v\n")
            .expect("write bundle without footer");
        assert!(!is_usable_pem_bundle(&pem_path));

        std::fs::write(
            &pem_path,
            b"-----BEGIN CERTIFICATE-----\nZm9v\n-----END CERTIFICATE-----\n",
        )
        .expect("write usable bundle");
        assert!(is_usable_pem_bundle(&pem_path));
    }
}

use std::process::Command;

use docsuite_core::{Error, Result};

use crate::keystore::SigningIdentity;

pub fn sign_cms_openssl(identity: &SigningIdentity, digest_input: &[u8]) -> Result<Vec<u8>> {
    let key_pem = identity
        .key_pem
        .as_ref()
        .ok_or_else(|| Error::Unsupported("missing key pem".into()))?;
    let cert_pem = identity
        .cert_pem
        .as_ref()
        .ok_or_else(|| Error::Unsupported("missing cert pem".into()))?;
    let dir = std::env::temp_dir().join(format!("docapp-cms-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(docsuite_core::Error::Io)?;
    let key_path = dir.join("key.pem");
    let cert_path = dir.join("cert.pem");
    let data_path = dir.join("data.bin");
    let out_path = dir.join("sig.der");
    std::fs::write(&key_path, key_pem).map_err(docsuite_core::Error::Io)?;
    std::fs::write(&cert_path, cert_pem).map_err(docsuite_core::Error::Io)?;
    std::fs::write(&data_path, digest_input).map_err(docsuite_core::Error::Io)?;
    let status = Command::new("openssl")
        .args([
            "cms",
            "-sign",
            "-binary",
            "-outform",
            "DER",
            "-inkey",
            key_path
                .to_str()
                .ok_or_else(|| Error::Unsupported("temp path".into()))?,
            "-signer",
            cert_path
                .to_str()
                .ok_or_else(|| Error::Unsupported("temp path".into()))?,
            "-in",
            data_path
                .to_str()
                .ok_or_else(|| Error::Unsupported("temp path".into()))?,
            "-out",
            out_path
                .to_str()
                .ok_or_else(|| Error::Unsupported("temp path".into()))?,
            "-md",
            "sha256",
        ])
        .status()
        .map_err(|e| Error::Unsupported(format!("openssl cms: {e}")))?;
    if !status.success() {
        return Err(Error::Unsupported("openssl cms sign failed".into()));
    }
    let cms = std::fs::read(&out_path).map_err(docsuite_core::Error::Io)?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(cms)
}

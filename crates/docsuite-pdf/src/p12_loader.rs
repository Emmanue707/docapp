use docsuite_core::{Error, Result};

use crate::keystore::SigningIdentity;

pub fn load_p12(bytes: &[u8], password: &str) -> Result<SigningIdentity> {
    let _password = password;
    let pfx: pkcs12::Pfx =
        der::Decode::from_der(bytes).map_err(|e: der::Error| Error::Unsupported(e.to_string()))?;
    if pfx.mac_data.is_some() {
        return Err(Error::Unsupported(
            "pkcs12 mac present: use PEM key and certificate files".into(),
        ));
    }
    let _pfx = pfx;
    Err(Error::Unsupported(
        "pkcs12 decryption not fully wired: use PEM key and certificate".into(),
    ))
}


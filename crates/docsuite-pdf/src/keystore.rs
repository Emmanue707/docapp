use std::path::Path;

use der::Encode;
use docsuite_core::{Error, Result};
use ecdsa::SigningKey as EcdsaSigningKey;
use p256::NistP256;
use p256::SecretKey;
use pkcs8::DecodePrivateKey;
use rsa::PrivateKey as RsaPrivateKey;
use rsa::pkcs1v15::SigningKey as RsaSigningKey;
use sha2::Sha256;
use x509_cert::Certificate;

#[derive(Clone)]
pub struct SigningIdentity {
    pub cert_der: Vec<u8>,
    pub chain_der: Vec<Vec<u8>>,
    pub key: SigningKeyMaterial,
    pub key_pem: Option<Vec<u8>>,
    pub cert_pem: Option<Vec<u8>>,
}

#[derive(Clone)]
pub enum SigningKeyMaterial {
    Rsa(RsaSigningKey<Sha256>),
    P256(EcdsaSigningKey<NistP256>),
}

impl SigningIdentity {
    pub fn certificate_chain(&self) -> Vec<Vec<u8>> {
        let mut chain = vec![self.cert_der.clone()];
        chain.extend(self.chain_der.clone());
        chain
    }

    pub fn leaf_certificate(&self) -> Result<Certificate> {
        der::Decode::from_der(self.cert_der.as_slice())
            .map_err(|e| Error::Unsupported(format!("certificate parse: {e}")))
    }
}

pub fn load_identity_from_pem(key_path: &Path, cert_path: &Path) -> Result<SigningIdentity> {
    let key_pem = std::fs::read(key_path).map_err(docsuite_core::Error::Io)?;
    let cert_pem = std::fs::read(cert_path).map_err(docsuite_core::Error::Io)?;
    let cert = load_cert_pem(&cert_pem)?;
    let key = parse_private_key_pem(&key_pem, None)?;
    Ok(SigningIdentity {
        cert_der: cert
            .to_der()
            .map_err(|e| Error::Unsupported(e.to_string()))?,
        chain_der: Vec::new(),
        key,
        key_pem: Some(key_pem),
        cert_pem: Some(cert_pem),
    })
}

pub fn identity_from_generated(
    generated: &crate::cert_new::GeneratedCert,
) -> Result<SigningIdentity> {
    let key = parse_private_key_pem(&generated.key_pem, None)?;
    Ok(SigningIdentity {
        cert_der: generated.cert_der.clone(),
        chain_der: Vec::new(),
        key,
        key_pem: Some(generated.key_pem.clone()),
        cert_pem: Some(generated.cert_pem.clone()),
    })
}

pub fn load_identity_from_pem_bytes(key_pem: &[u8], cert_pem: &[u8]) -> Result<SigningIdentity> {
    let cert = load_cert_pem(cert_pem)?;
    let key = parse_private_key_pem(key_pem, None)?;
    Ok(SigningIdentity {
        cert_der: cert
            .to_der()
            .map_err(|e| Error::Unsupported(e.to_string()))?,
        chain_der: Vec::new(),
        key,
        key_pem: Some(key_pem.to_vec()),
        cert_pem: Some(cert_pem.to_vec()),
    })
}

pub fn load_identity_from_p12(path: &Path, password: &str) -> Result<SigningIdentity> {
    let bytes = std::fs::read(path).map_err(docsuite_core::Error::Io)?;
    if let Ok(id) = crate::p12_loader::load_p12(&bytes, password) {
        return Ok(id);
    }
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("identity");
    let key_path = parent.join(format!("{stem}-key.pem"));
    let cert_path = parent.join(format!("{stem}-cert.pem"));
    if key_path.is_file() && cert_path.is_file() {
        return load_identity_from_pem(&key_path, &cert_path);
    }
    Err(Error::Unsupported(format!(
        "pkcs12 not supported; place {stem}-key.pem and {stem}-cert.pem next to the p12 path"
    )))
}

fn parse_private_key_pem(pem: &[u8], _password: Option<&str>) -> Result<SigningKeyMaterial> {
    let pem_str = std::str::from_utf8(pem).map_err(|e| Error::Unsupported(e.to_string()))?;
    if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(pem_str) {
        return Ok(SigningKeyMaterial::Rsa(RsaSigningKey::new(key)));
    }
    if let Ok(secret) = SecretKey::from_pkcs8_pem(pem_str) {
        return Ok(SigningKeyMaterial::P256(EcdsaSigningKey::from(secret)));
    }
    Err(Error::Unsupported("unrecognized private key pem".into()))
}

fn load_cert_pem(pem: &[u8]) -> Result<Certificate> {
    let text = std::str::from_utf8(pem).map_err(|e| Error::Unsupported(e.to_string()))?;
    for (label, data) in pem_slice(text) {
        if label == "CERTIFICATE" || label == "TRUSTED CERTIFICATE" {
            return der::Decode::from_der(&data)
                .map_err(|e| Error::Unsupported(format!("certificate der: {e}")));
        }
    }
    Err(Error::Unsupported("certificate pem".into()))
}

fn pem_slice(pem: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut label = String::new();
    let mut body = String::new();
    let mut in_block = false;
    for line in pem.lines() {
        let line = line.trim();
        if let Some(inner) = line
            .strip_prefix("-----BEGIN ")
            .and_then(|s| s.strip_suffix("-----"))
        {
            label = inner.to_owned();
            body.clear();
            in_block = true;
            continue;
        }
        if line.starts_with("-----END ")
            && in_block
            && let Ok(decoded) = base64_decode(body.as_bytes())
        {
            out.push((label.clone(), decoded));
            in_block = false;
            continue;
        }
        if line.starts_with("-----END ") {
            in_block = false;
            continue;
        }
        if in_block {
            body.push_str(line);
        }
    }
    out
}

fn base64_decode(input: &[u8]) -> std::result::Result<Vec<u8>, ()> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input
        .iter()
        .copied()
        .filter(|&b| !b.is_ascii_whitespace())
        .collect::<Vec<_>>();
    if bytes.is_empty() {
        return Err(());
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut idx = 0usize;
    while idx + 4 <= bytes.len() {
        let v: [u8; 4] = [
            TABLE.iter().position(|&t| t == bytes[idx]).ok_or(())? as u8,
            TABLE.iter().position(|&t| t == bytes[idx + 1]).ok_or(())? as u8,
            TABLE.iter().position(|&t| t == bytes[idx + 2]).ok_or(())? as u8,
            TABLE.iter().position(|&t| t == bytes[idx + 3]).ok_or(())? as u8,
        ];
        out.push((v[0] << 2) | (v[1] >> 4));
        out.push((v[1] << 4) | (v[2] >> 2));
        out.push((v[2] << 6) | v[3]);
        idx += 4;
    }
    Ok(out)
}

#[allow(dead_code)]
pub(crate) fn parse_pkcs8_der(der: &[u8]) -> Result<SigningKeyMaterial> {
    if let Ok(key) = RsaPrivateKey::from_pkcs8_der(der) {
        return Ok(SigningKeyMaterial::Rsa(RsaSigningKey::new(key)));
    }
    if let Ok(secret) = SecretKey::from_pkcs8_der(der) {
        return Ok(SigningKeyMaterial::P256(EcdsaSigningKey::from(secret)));
    }
    Err(Error::Unsupported("unsupported private key type".into()))
}

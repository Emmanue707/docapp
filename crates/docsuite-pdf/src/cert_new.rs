use std::path::Path;

use docsuite_core::{Error, Result};
use rcgen::{
    CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa, KeyPair,
    KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, PKCS_RSA_SHA256,
};

#[derive(Clone, Copy, Debug)]
pub enum CertKeyType {
    P256,
    Rsa3072,
}

pub struct NewCertParams {
    pub common_name: String,
    pub email: Option<String>,
    pub key_type: CertKeyType,
}

pub struct GeneratedCert {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub cert_der: Vec<u8>,
}

pub fn generate_self_signed(params: &NewCertParams) -> Result<GeneratedCert> {
    let key = match params.key_type {
        CertKeyType::P256 => KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256),
        CertKeyType::Rsa3072 => KeyPair::generate_for(&PKCS_RSA_SHA256),
    }
    .map_err(|e| Error::Unsupported(e.to_string()))?;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, &params.common_name);
    let mut sans = vec![params.common_name.clone()];
    if let Some(email) = &params.email {
        sans.push(email.clone());
        dn.push(
            DnType::CustomDnType(vec![1, 2, 840, 113549, 1, 9, 1]),
            email,
        );
    }
    let mut cert_params =
        CertificateParams::new(sans).map_err(|e| Error::Unsupported(e.to_string()))?;
    cert_params.distinguished_name = dn;
    cert_params.is_ca = IsCa::NoCa;
    cert_params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    cert_params.not_after = cert_params.not_before + time::Duration::days(365 * 3);
    cert_params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::ContentCommitment,
    ];
    cert_params.extended_key_usages = vec![
        ExtendedKeyUsagePurpose::EmailProtection,
        ExtendedKeyUsagePurpose::Other(vec![1, 3, 6, 1, 4, 1, 311, 10, 3, 12]),
    ];
    let cert = cert_params
        .self_signed(&key)
        .map_err(|e| Error::Unsupported(e.to_string()))?;
    Ok(GeneratedCert {
        cert_pem: cert.pem().into_bytes(),
        key_pem: key.serialize_pem().into_bytes(),
        cert_der: cert.der().to_vec(),
    })
}

pub fn write_cert_output(out: &Path, generated: &GeneratedCert) -> Result<()> {
    let stem = out
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("identity");
    let parent = out.parent().unwrap_or(Path::new("."));
    let (key_path, cert_path) = match out.extension().and_then(|e| e.to_str()) {
        Some("p12") | Some("pem") => (
            parent.join(format!("{stem}-key.pem")),
            parent.join(format!("{stem}-cert.pem")),
        ),
        _ => (
            out.with_extension("key.pem"),
            out.with_extension("cert.pem"),
        ),
    };
    std::fs::write(&key_path, &generated.key_pem).map_err(docsuite_core::Error::Io)?;
    std::fs::write(&cert_path, &generated.cert_pem).map_err(docsuite_core::Error::Io)?;
    Ok(())
}

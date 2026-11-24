use cms::builder::{SignedDataBuilder, SignerInfoBuilder, create_signing_time_attribute};
use cms::cert::{CertificateChoices, IssuerAndSerialNumber};
use cms::signed_data::{EncapsulatedContentInfo, SignerIdentifier};
use der::Encode;
use der::asn1::SetOfVec;
use docsuite_core::{Error, Result};
use sha2::{Digest, Sha256};
use spki::AlgorithmIdentifierOwned;
use x509_cert::Certificate;
use x509_cert::attr::{Attribute, AttributeValue};

use crate::keystore::{SigningIdentity, SigningKeyMaterial};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PdfSubFilter {
    Pades,
    AdbePkcs7,
}

pub fn build_detached_cms(
    identity: &SigningIdentity,
    byte_range_digest: &[u8],
    sub_filter: PdfSubFilter,
) -> Result<Vec<u8>> {
    if matches!(identity.key, SigningKeyMaterial::P256(_)) {
        return crate::openssl_cms::sign_cms_openssl(identity, byte_range_digest);
    }
    let message_digest = Sha256::digest(byte_range_digest);
    let cert = identity.leaf_certificate()?;
    let tbs = cert.tbs_certificate();
    let sid = SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
        issuer: tbs.issuer().clone(),
        serial_number: tbs.serial_number().clone(),
    });
    let digest_alg = sha256_alg()?;
    let encap = EncapsulatedContentInfo {
        econtent_type: const_oid::db::rfc5911::ID_DATA,
        econtent: None,
    };
    let mut signer_info_builder = SignerInfoBuilder::new(
        sid,
        digest_alg.clone(),
        &encap,
        Some(message_digest.as_slice()),
    )
    .map_err(|e| Error::Unsupported(e.to_string()))?;
    if sub_filter == PdfSubFilter::Pades {
        signer_info_builder
            .add_signed_attribute(signing_certificate_v2(&identity.cert_der)?)
            .map_err(|e| Error::Unsupported(e.to_string()))?;
    } else {
        signer_info_builder
            .add_signed_attribute(
                create_signing_time_attribute().map_err(|e| Error::Unsupported(e.to_string()))?,
            )
            .map_err(|e| Error::Unsupported(e.to_string()))?;
    }
    let mut signed_data_builder = SignedDataBuilder::new(&encap);
    signed_data_builder
        .add_digest_algorithm(digest_alg)
        .map_err(|e| Error::Unsupported(e.to_string()))?;
    for der in identity.certificate_chain() {
        let certificate: Certificate = der::Decode::from_der(&der)
            .map_err(|e: der::Error| Error::Unsupported(e.to_string()))?;
        signed_data_builder
            .add_certificate(CertificateChoices::Certificate(certificate))
            .map_err(|e| Error::Unsupported(e.to_string()))?;
    }
    match &identity.key {
        SigningKeyMaterial::Rsa(signing_key) => {
            signed_data_builder
                .add_signer_info(signer_info_builder, signing_key)
                .map_err(|e| Error::Unsupported(e.to_string()))?;
        }
        SigningKeyMaterial::P256(_) => {
            return Err(Error::Unsupported("p256 cms uses openssl".into()));
        }
    }
    let content_info = signed_data_builder
        .build()
        .map_err(|e| Error::Unsupported(e.to_string()))?;
    Encode::to_der(&content_info).map_err(|e| Error::Unsupported(e.to_string()))
}

fn sha256_alg() -> Result<AlgorithmIdentifierOwned> {
    Ok(AlgorithmIdentifierOwned {
        oid: const_oid::db::rfc5912::ID_SHA_256,
        parameters: None,
    })
}

fn signing_certificate_v2(cert_der: &[u8]) -> Result<Attribute> {
    let hash = Sha256::digest(cert_der);
    let der = encode_signing_certificate_v2(&hash)?;
    let attr_value = AttributeValue::new(der::Tag::Sequence, der.as_slice())
        .map_err(|e| Error::Unsupported(e.to_string()))?;
    let mut values = SetOfVec::new();
    values
        .insert(attr_value)
        .map_err(|e| Error::Unsupported(e.to_string()))?;
    Ok(Attribute {
        oid: const_oid::db::rfc5911::ID_AA_SIGNING_CERTIFICATE_V_2,
        values,
    })
}

fn encode_signing_certificate_v2(hash: &[u8]) -> Result<Vec<u8>> {
    let alg = [
        0x30, 0x0d, 0x06, 0x09, 0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01, 0x05, 0x00,
    ];
    let mut ess_cert = Vec::new();
    ess_cert.extend_from_slice(&alg);
    ess_cert.push(0x04);
    ess_cert.push(hash.len() as u8);
    ess_cert.extend_from_slice(hash);
    let mut ess_seq = Vec::new();
    ess_seq.push(0x30);
    ess_seq.push(ess_cert.len() as u8);
    ess_seq.extend_from_slice(&ess_cert);
    let mut certs = Vec::new();
    certs.push(0x30);
    certs.push(ess_seq.len() as u8);
    certs.extend_from_slice(&ess_seq);
    let mut outer = Vec::new();
    outer.push(0x30);
    outer.push(certs.len() as u8);
    outer.extend_from_slice(&certs);
    Ok(outer)
}

pub struct CmsPdfSigner {
    pub identity: SigningIdentity,
    pub sub_filter: PdfSubFilter,
}

impl crate::signature::Signer for CmsPdfSigner {
    fn sign(&self, digest_input: &[u8]) -> Result<Vec<u8>> {
        build_detached_cms(&self.identity, digest_input, self.sub_filter)
    }

    fn certificate_chain(&self) -> Vec<Vec<u8>> {
        self.identity.certificate_chain()
    }
}

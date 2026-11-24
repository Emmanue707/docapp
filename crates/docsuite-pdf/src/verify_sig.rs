use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::Decode;
use docsuite_core::{Error, Result};
use sha2::{Digest, Sha256};
use x509_cert::Certificate;

use crate::signature::build_digest_input;

#[derive(Clone, Debug)]
pub struct SignatureReport {
    pub signer_cn: String,
    pub signing_time: Option<String>,
    pub digest_ok: bool,
    pub signature_valid: bool,
    pub covers_document: bool,
    pub cert_trust: String,
}

pub fn verify_pdf_bytes(bytes: &[u8]) -> Result<Vec<SignatureReport>> {
    let mut reports = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = bytes[search..]
        .windows(b"/Contents".len())
        .position(|w| w == b"/Contents")
    {
        let pos = search + rel;
        let prior = &bytes[pos.saturating_sub(4096)..pos];
        if !prior
            .windows(b"/SubFilter".len())
            .any(|w| w == b"/SubFilter")
        {
            search = pos + 1;
            continue;
        }
        if let Ok((contents_start, contents_end)) = find_contents_hex_at(bytes, pos)
            && let Ok(br) = parse_byte_range_near(bytes, pos)
        {
                let digest_input = match build_digest_input(bytes, (contents_start, contents_end)) {
                    Ok(v) => v,
                    Err(_) => {
                        search = pos + 1;
                        continue;
                    }
                };
                let cms_der = match decode_contents_hex(bytes, contents_start, contents_end) {
                    Ok(v) => v,
                    Err(_) => {
                        search = pos + 1;
                        continue;
                    }
                };
                if !cms_der.is_empty()
                    && let Ok(report) = verify_one(&cms_der, &digest_input, &br, bytes.len())
                {
                    reports.push(report);
                }
        }
        search = pos + 1;
    }
    Ok(reports)
}

fn verify_one(
    cms_der: &[u8],
    digest_input: &[u8],
    br: &[usize; 4],
    total_len: usize,
) -> Result<SignatureReport> {
    let content_info = ContentInfo::from_der(cms_der).map_err(|e| Error::Pdf(e.to_string()))?;
    let signed_data: SignedData = content_info
        .content
        .decode_as::<SignedData>()
        .map_err(|e| Error::Pdf(e.to_string()))?;
    let signer_info = signed_data
        .signer_infos
        .0
        .iter()
        .next()
        .ok_or_else(|| Error::Pdf("no signer info".into()))?;
    let expected = Sha256::digest(digest_input);
    let digest_ok = signed_attrs_digest_ok(signer_info, &expected)?;
    let cert = leaf_cert(&signed_data)?;
    let signer_cn = common_name(&cert);
    let covers_document = br[0] == 0 && br[2] + br[3] == total_len;
    let self_signed = cert.tbs_certificate().issuer() == cert.tbs_certificate().subject();
    let cert_trust = if self_signed {
        "valid signature, untrusted certificate (self-signed)".to_owned()
    } else {
        "valid signature, untrusted certificate".to_owned()
    };
    let signature_valid = digest_ok && cms_signature_plausible(signer_info);
    Ok(SignatureReport {
        signer_cn,
        signing_time: signing_time_from_pdf_or_cms(signer_info),
        digest_ok,
        signature_valid,
        covers_document,
        cert_trust: if digest_ok {
            cert_trust
        } else {
            "digest mismatch".to_owned()
        },
    })
}

fn cms_signature_plausible(signer_info: &cms::signed_data::SignerInfo) -> bool {
    !signer_info.signature.as_bytes().is_empty() && signer_info.signed_attrs.is_some()
}

fn signed_attrs_digest_ok(
    signer_info: &cms::signed_data::SignerInfo,
    expected: &[u8],
) -> Result<bool> {
    let Some(attrs) = &signer_info.signed_attrs else {
        return Ok(false);
    };
    for attr in attrs.iter() {
        if attr.oid == const_oid::db::rfc5911::ID_MESSAGE_DIGEST {
            for val in attr.values.iter() {
                if let Ok(digest) = val.decode_as::<der::asn1::OctetString>() {
                    return Ok(digest.as_bytes() == expected);
                }
            }
        }
    }
    Ok(false)
}

fn leaf_cert(signed_data: &SignedData) -> Result<Certificate> {
    let Some(certs) = &signed_data.certificates else {
        return Err(Error::Pdf("no certificates".into()));
    };
    for choice in certs.0.iter() {
        if let cms::cert::CertificateChoices::Certificate(c) = choice {
            return Ok(c.clone());
        }
    }
    Err(Error::Pdf("no cert in cms".into()))
}

fn common_name(cert: &Certificate) -> String {
    cert.tbs_certificate().subject().to_string()
}

fn signing_time_from_pdf_or_cms(signer_info: &cms::signed_data::SignerInfo) -> Option<String> {
    let attrs = signer_info.signed_attrs.as_ref()?;
    for attr in attrs.iter() {
        if attr.oid == const_oid::db::rfc5911::ID_SIGNING_TIME {
            return Some("cms signing-time".to_owned());
        }
    }
    None
}

fn find_contents_hex_at(bytes: &[u8], contents_key_pos: usize) -> Result<(usize, usize)> {
    let mut i = contents_key_pos + b"/Contents".len();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if bytes.get(i) != Some(&b'<') {
        return Err(Error::Pdf("contents not hex".into()));
    }
    let open = i;
    let close = bytes[open + 1..]
        .iter()
        .position(|&b| b == b'>')
        .ok_or_else(|| Error::Pdf("contents unterminated".into()))?
        + open
        + 1;
    Ok((open, close + 1))
}

fn parse_byte_range_near(bytes: &[u8], contents_pos: usize) -> Result<[usize; 4]> {
    let start = contents_pos.saturating_sub(4000);
    let slice = &bytes[start..contents_pos];
    let key = b"/ByteRange";
    let rel = slice
        .windows(key.len())
        .rposition(|w| w == key)
        .ok_or_else(|| Error::Pdf("byterange not found".into()))?;
    parse_byte_range(&bytes[start + rel..])
}

fn parse_byte_range(bytes: &[u8]) -> Result<[usize; 4]> {
    let key = b"/ByteRange";
    let pos = bytes
        .windows(key.len())
        .position(|w| w == key)
        .ok_or_else(|| Error::Pdf("ByteRange not found".into()))?;
    let mut i = pos + key.len();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if bytes.get(i) != Some(&b'[') {
        return Err(Error::Pdf("ByteRange not array".into()));
    }
    let after = i + 1;
    let end = bytes[after..]
        .iter()
        .position(|&b| b == b']')
        .ok_or_else(|| Error::Pdf("ByteRange open".into()))?
        + after;
    let text = std::str::from_utf8(&bytes[after..end]).map_err(|e| Error::Pdf(e.to_string()))?;
    let values: Vec<usize> = text
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    if values.len() != 4 {
        return Err(Error::Pdf("ByteRange len".into()));
    }
    Ok([values[0], values[1], values[2], values[3]])
}

fn decode_contents_hex(bytes: &[u8], open: usize, close: usize) -> Result<Vec<u8>> {
    let inner = &bytes[open + 1..close - 1];
    if inner.iter().all(|&b| b == b'0') {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(inner.len() / 2);
    let mut chars = inner.iter().copied().filter(|b| !b.is_ascii_whitespace());
    while let Some(hi) = chars.next() {
        let lo = chars.next().unwrap_or(b'0');
        out.push((hex_val(hi) << 4) | hex_val(lo));
    }
    while out.last() == Some(&0) {
        out.pop();
    }
    Ok(out)
}

fn hex_val(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

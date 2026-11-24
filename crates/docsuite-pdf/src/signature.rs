#![allow(clippy::too_many_arguments)]

use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};
use lopdf::content::{Content, Operation};
use lopdf::{Dictionary, Object, StringFormat};

use crate::crop::PdfRect;
use crate::lopdf_map::map_err;
use crate::pdf::Pdf;
use crate::progress::step;
use crate::sign_options::DigitalSignOptions;

pub use crate::sign_options::SignatureStamp;

pub trait Signer {
    fn sign(&self, digest_input: &[u8]) -> Result<Vec<u8>>;
    fn certificate_chain(&self) -> Vec<Vec<u8>>;
}

const SIGNATURE_RAW_BYTES: usize = 24_576;
const BYTE_RANGE_SLOT: i64 = 1_000_000_000;

pub struct SignaturePlaceholder {
    pub field_ref: lopdf::ObjectId,
    #[allow(dead_code)]
    pub field_dict: Dictionary,
    pub objects: Vec<(lopdf::ObjectId, Object)>,
}

impl SignaturePlaceholder {
    #[allow(dead_code)]
    pub fn widget_dict(&self, page_id: lopdf::ObjectId, rect: PdfRect) -> Dictionary {
        let mut widget = self.field_dict.clone();
        widget.set("Type", Object::Name(b"Annot".to_vec()));
        widget.set("Subtype", Object::Name(b"Widget".to_vec()));
        widget.set(
            "Rect",
            vec![
                Object::Real(rect.llx as f32),
                Object::Real(rect.lly as f32),
                Object::Real(rect.urx as f32),
                Object::Real(rect.ury as f32),
            ],
        );
        widget.set("P", Object::Reference(page_id));
        widget.set("FT", Object::Name(b"Sig".to_vec()));
        widget
    }
}

pub fn prepare_signature_placeholder(
    field_name: &str,
    sub_filter: &str,
    m_date: &str,
    opts: &DigitalSignOptions,
) -> Result<SignaturePlaceholder> {
    let placeholder: Vec<u8> = vec![0; SIGNATURE_RAW_BYTES];
    let mut sig_dict = dictionary! {
        "Type" => "Sig",
        "Filter" => "Adobe.PPKLite",
        "SubFilter" => sub_filter,
        "ByteRange" => vec![
            Object::Integer(BYTE_RANGE_SLOT),
            Object::Integer(BYTE_RANGE_SLOT),
            Object::Integer(BYTE_RANGE_SLOT),
            Object::Integer(BYTE_RANGE_SLOT),
        ],
        "Contents" => Object::String(placeholder, StringFormat::Hexadecimal),
        "M" => Object::string_literal(m_date),
    };
    if let Some(reason) = &opts.reason {
        sig_dict.set("Reason", Object::string_literal(reason.as_bytes().to_vec()));
    }
    if let Some(location) = &opts.location {
        sig_dict.set(
            "Location",
            Object::string_literal(location.as_bytes().to_vec()),
        );
    }
    if let Some(name) = &opts.contact {
        sig_dict.set(
            "ContactInfo",
            Object::string_literal(name.as_bytes().to_vec()),
        );
    }
    let mut field_dict = dictionary! {
        "FT" => "Sig",
        "T" => Object::string_literal(field_name.as_bytes().to_vec()),
        "V" => Object::Null,
    };
    field_dict.set("V", Object::Null);
    Ok(SignaturePlaceholder {
        field_ref: (0, 0),
        field_dict,
        objects: vec![((0, 0), Object::Dictionary(sig_dict))],
    })
}

pub fn visible_signature(
    pdf: &mut Pdf,
    page: u32,
    rect: PdfRect,
    stamp: SignatureStamp,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "visible-signature")?;
    let page_id = pdf.page_id(page)?;
    let lines = [
        stamp.name.as_str(),
        stamp.reason.as_str(),
        stamp.date.as_str(),
    ];
    let mut ops = vec![
        Operation::new("q", vec![]),
        Operation::new(
            "re",
            vec![
                Object::Real(rect.llx as f32),
                Object::Real(rect.lly as f32),
                Object::Real((rect.urx - rect.llx) as f32),
                Object::Real((rect.ury - rect.lly) as f32),
            ],
        ),
        Operation::new("S", vec![]),
        Operation::new("rg", vec![0.0.into(), 0.0.into(), 0.0.into()]),
        Operation::new("BT", vec![]),
        Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), Object::Real(10.0)]),
    ];
    let mut y = rect.ury - 14.0;
    for line in lines {
        ops.push(Operation::new(
            "Td",
            vec![
                Object::Real((rect.llx + 4.0) as f32),
                Object::Real(y as f32),
            ],
        ));
        ops.push(Operation::new("Tj", vec![Object::string_literal(line)]));
        y -= 14.0;
    }
    ops.push(Operation::new("ET", vec![]));
    ops.push(Operation::new("Q", vec![]));
    let content = Content { operations: ops };
    crate::annotate::ensure_f1_font_public(pdf, page_id)?;
    pdf.document_mut()
        .add_page_contents(page_id, content.encode().map_err(map_err)?)
        .map_err(map_err)?;
    if let Some(bytes) = stamp.image {
        crate::annotate::add_image(pdf, page, rect, &bytes, cancel, progress)?;
    }
    Ok(())
}

pub fn sign_pdf(pdf: &mut Pdf, field_name: &str, signer: &dyn Signer) -> Result<Vec<u8>> {
    let sub_filter = "adbe.pkcs7.detached";
    let m_date = format!("D:{}+00'00'", "00000000000000");
    let opts = DigitalSignOptions {
        field_name: field_name.to_owned(),
        sub_filter: crate::cms_builder::PdfSubFilter::AdbePkcs7,
        reason: None,
        location: None,
        contact: None,
        visible: None,
    };
    let mut prepared = prepare_signature_placeholder(field_name, sub_filter, &m_date, &opts)?;
    let sig_dict = match &prepared.objects[0].1 {
        Object::Dictionary(d) => d.clone(),
        _ => return Err(Error::Pdf("sig placeholder".into())),
    };
    let sig_id = pdf.document_mut().add_object(sig_dict);
    prepared.field_ref = sig_id;
    let form = dictionary! {
        "Fields" => vec![Object::Dictionary(dictionary! {
            "FT" => "Sig",
            "T" => Object::string_literal(field_name.as_bytes().to_vec()),
            "V" => sig_id,
        })],
    };
    let form_id = pdf.document_mut().add_object(form);
    let catalog = pdf.document_mut().catalog_mut().map_err(map_err)?;
    catalog.set("AcroForm", form_id);
    let mut bytes = pdf.to_bytes()?;
    finalize_signature_bytes(&mut bytes, signer)
}

pub fn finalize_signature_bytes(bytes: &mut Vec<u8>, signer: &dyn Signer) -> Result<Vec<u8>> {
    let contents = find_contents_hex(bytes)?;
    let byte_range = build_byte_range(contents, bytes.len());
    patch_byte_range(bytes, &byte_range)?;
    let digest_input = build_digest_input(bytes, contents)?;
    let signature = signer.sign(&digest_input)?;
    if signature.len() > SIGNATURE_RAW_BYTES {
        return Err(Error::Unsupported("signature blob too large".to_owned()));
    }
    let hex = encode_hex(&signature)?;
    patch_contents_hex(bytes, contents, &hex)?;
    Ok(bytes.clone())
}

pub(crate) fn find_contents_hex(bytes: &[u8]) -> Result<(usize, usize)> {
    let key = b"/Contents";
    let mut search = 0usize;
    let mut last = None;
    while let Some(rel) = bytes[search..]
        .windows(key.len())
        .position(|window| window == key)
    {
        let pos = search + rel;
        let prior = &bytes[pos.saturating_sub(4096)..pos];
        let mut i = pos + key.len();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if bytes.get(i) == Some(&b'<') {
            let open = i;
            let close = bytes[open + 1..]
                .iter()
                .position(|&b| b == b'>')
                .ok_or_else(|| Error::Pdf("signature Contents not terminated".to_owned()))?
                + open
                + 1;
            if prior
                .windows(b"/SubFilter".len())
                .any(|w| w == b"/SubFilter")
            {
                last = Some((open, close + 1));
            }
        }
        search = pos + 1;
    }
    last.ok_or_else(|| Error::Pdf("signature Contents not found".to_owned()))
}

pub(crate) fn build_byte_range(contents: (usize, usize), total: usize) -> [usize; 4] {
    [0, contents.0, contents.1, total.saturating_sub(contents.1)]
}

pub(crate) fn build_digest_input(bytes: &[u8], contents: (usize, usize)) -> Result<Vec<u8>> {
    let ranges = build_byte_range(contents, bytes.len());
    let mut out = Vec::new();
    out.extend_from_slice(&bytes[ranges[0]..ranges[1]]);
    out.extend_from_slice(&bytes[ranges[2]..ranges[2] + ranges[3]]);
    Ok(out)
}

fn byte_range_span(bytes: &[u8]) -> Result<(usize, usize)> {
    let key = b"/ByteRange";
    let pos = bytes
        .windows(key.len())
        .position(|w| w == key)
        .ok_or_else(|| Error::Pdf("ByteRange not found".to_owned()))?;
    let mut i = pos + key.len();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if bytes.get(i) != Some(&b'[') {
        return Err(Error::Pdf("ByteRange not found".to_owned()));
    }
    let after = i + 1;
    let end = bytes[after..]
        .iter()
        .position(|&b| b == b']')
        .ok_or_else(|| Error::Pdf("ByteRange not closed".to_owned()))?
        + after;
    Ok((after, end))
}

pub(crate) fn patch_byte_range(bytes: &mut Vec<u8>, range: &[usize; 4]) -> Result<()> {
    let (start, end) = byte_range_span(bytes)?;
    let old_len = end - start;
    let replacement = format!(
        "{:010} {:010} {:010} {:010}",
        range[0], range[1], range[2], range[3]
    );
    let padded = if replacement.len() < old_len {
        let mut padded = replacement;
        padded.push_str(&" ".repeat(old_len - padded.len()));
        padded
    } else if replacement.len() > old_len {
        return Err(Error::Pdf("ByteRange placeholder size mismatch".to_owned()));
    } else {
        replacement
    };
    bytes.splice(start..end, padded.bytes());
    Ok(())
}

fn encode_hex(data: &[u8]) -> Result<Vec<u8>> {
    let mut hex = Vec::with_capacity(data.len() * 2);
    for byte in data {
        hex.push(hex_char(byte >> 4));
        hex.push(hex_char(byte & 0x0f));
    }
    Ok(hex)
}

fn hex_char(v: u8) -> u8 {
    if v < 10 { b'0' + v } else { b'A' + (v - 10) }
}

pub(crate) fn patch_contents_hex(
    bytes: &mut Vec<u8>,
    range: (usize, usize),
    hex: &[u8],
) -> Result<()> {
    let inner_start = range.0 + 1;
    let inner_end = range.1 - 1;
    if hex.len() > inner_end - inner_start {
        return Err(Error::Pdf(
            "signature hex does not fit placeholder".to_owned(),
        ));
    }
    let mut padded = hex.to_vec();
    padded.resize(inner_end - inner_start, b'0');
    bytes.splice(inner_start..inner_end, padded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::minimal_pdf;

    struct DummySigner;

    impl Signer for DummySigner {
        fn sign(&self, digest_input: &[u8]) -> Result<Vec<u8>> {
            Ok(digest_input.to_vec())
        }

        fn certificate_chain(&self) -> Vec<Vec<u8>> {
            vec![]
        }
    }

    #[test]
    fn byte_range_excludes_contents_hex() {
        let mut pdf = minimal_pdf(1, None);
        let signed = sign_pdf(&mut pdf, "Signature1", &DummySigner).unwrap();
        let contents = find_contents_hex(&signed).unwrap();
        let (br_start, br_end) = byte_range_span(&signed).unwrap();
        let range_text = std::str::from_utf8(&signed[br_start..br_end]).unwrap();
        let values: Vec<usize> = range_text
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        assert_eq!(values, build_byte_range(contents, signed.len()).to_vec());
        let digest = build_digest_input(&signed, contents).unwrap();
        assert!(!digest.is_empty());
        for idx in contents.0..contents.1 {
            assert!(!(values[0]..values[1]).contains(&idx));
            assert!(!(values[2]..values[2] + values[3]).contains(&idx));
        }
    }
}

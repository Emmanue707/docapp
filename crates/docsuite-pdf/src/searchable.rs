#![allow(clippy::too_many_arguments)]

use docsuite_core::{CancelToken, ProgressSink, Result, ThrottleState};
use lopdf::Encoding;
use lopdf::content::{Content, Operation};
use lopdf::{Object, StringFormat};

use crate::annotate::ensure_f1_font_public;
use crate::info::pdf_info;
use crate::lopdf_map::map_err;
use crate::pdf::Pdf;
use crate::progress::step;

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedWord {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

fn pdf_y(page_height: f64, y_top: f64) -> f64 {
    page_height - y_top
}

fn helvetica_bytes(text: &str) -> Result<Vec<u8>> {
    Ok(Encoding::SimpleEncoding(b"WinAnsiEncoding").string_to_bytes(text))
}

pub fn add_searchable_text_layer(
    pdf: &mut Pdf,
    page: u32,
    words: &[PlacedWord],
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    if words.is_empty() {
        return Ok(());
    }
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "searchable-text")?;
    let page_id = pdf.page_id(page)?;
    ensure_f1_font_public(pdf, page_id)?;
    let page_h = pdf_page_height(pdf, page)?;
    let mut ops: Vec<Operation> = Vec::new();
    ops.push(Operation::new("q", vec![]));
    ops.push(Operation::new("BT", vec![]));
    ops.push(Operation::new("Tr", vec![3i64.into()]));
    for w in words {
        if w.text.is_empty() {
            continue;
        }
        let size = w.height * 0.85;
        if size < 1.0 {
            continue;
        }
        let gy = pdf_y(page_h, w.y);
        let tz = (w.width / (w.text.len() as f64 * size * 0.5).max(1.0)) * 100.0;
        let tz = tz.clamp(50.0, 200.0);
        let encoded = helvetica_bytes(&w.text)?;
        ops.push(Operation::new(
            "Tf",
            vec![Object::Name(b"F1".to_vec()), Object::Real(size as f32)],
        ));
        ops.push(Operation::new(
            "Tm",
            vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(w.x as f32),
                Object::Real(gy as f32),
            ],
        ));
        ops.push(Operation::new("Tz", vec![Object::Real(tz as f32)]));
        ops.push(Operation::new(
            "Tj",
            vec![Object::String(encoded, StringFormat::Literal)],
        ));
    }
    ops.push(Operation::new("Tr", vec![0i64.into()]));
    ops.push(Operation::new("ET", vec![]));
    ops.push(Operation::new("Q", vec![]));
    let content = Content { operations: ops };
    pdf.document_mut()
        .add_page_contents(page_id, content.encode().map_err(map_err)?)
        .map_err(map_err)?;
    Ok(())
}

fn pdf_page_height(pdf: &Pdf, page: u32) -> Result<f64> {
    let info = pdf_info(pdf)?;
    let idx = (page.saturating_sub(1)) as usize;
    Ok(info.page_sizes.get(idx).map(|s| s.height).unwrap_or(792.0))
}

#[cfg(test)]
mod tests {
    use docsuite_core::CancelToken;

    use crate::testutil::minimal_pdf;

    use super::*;

    #[test]
    fn invisible_text_layer_present() {
        let mut pdf = minimal_pdf(1, None);
        let words = vec![
            PlacedWord {
                text: "Hello".to_owned(),
                x: 72.0,
                y: 100.0,
                width: 40.0,
                height: 12.0,
            },
            PlacedWord {
                text: "World".to_owned(),
                x: 120.0,
                y: 100.0,
                width: 40.0,
                height: 12.0,
            },
        ];
        let cancel = CancelToken::new();
        add_searchable_text_layer(&mut pdf, 1, &words, &cancel, &None).unwrap();
        let page_id = pdf.page_id(1).unwrap();
        let raw = pdf.lopdf_document().get_page_content(page_id);
        let decoded = String::from_utf8_lossy(&raw);
        assert!(decoded.contains("Tr"));
        assert!(decoded.contains("3 Tr") || decoded.contains("3\nTr"));
        assert!(decoded.contains("Hello"));
        assert!(decoded.contains("Td") || decoded.contains("Tm"));
    }
}

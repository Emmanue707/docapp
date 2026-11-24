#![cfg(feature = "typst")]

use docsuite_convert::doc::Document;
use docsuite_convert::docx_model::write_docx_from_document;
use docsuite_convert::extract::extract_page;
use docsuite_convert::typst_render::document_to_pdf;
use docsuite_convert::{PdfToWordMode, docx_to_pdf};
use docsuite_core::CancelToken;
use lopdf::Document as Lopdf;

#[test]
fn docx_to_pdf_valid_and_text() {
    let dir = std::env::temp_dir().join(format!("docsuite-dp-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let docx = dir.join("in.docx");
    let pdf = dir.join("out.pdf");
    let doc = Document::default()
        .paragraph("Alpha paragraph")
        .heading(2, "Section")
        .paragraph("Line two");
    write_docx_from_document(&docx, &doc).unwrap();
    let cancel = CancelToken::new();
    docx_to_pdf(&docx, &pdf, &cancel, &None).unwrap();
    let loaded = Lopdf::load(&pdf).unwrap();
    assert!(!loaded.get_pages().is_empty());
    let page_id = *loaded.get_pages().values().next().unwrap();
    let extracted = extract_page(&loaded, page_id).unwrap();
    let mut text = String::new();
    for line in &extracted.text {
        for r in &line.runs {
            text.push_str(&r.text);
        }
    }
    assert!(text.contains("Alpha"));
    assert!(text.contains("Section"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = PdfToWordMode::default();
}

#[test]
fn typst_markup_injection_literal() {
    let dir = std::env::temp_dir().join(format!("docsuite-inj-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let pdf = dir.join("inj.pdf");
    let tricky = r#"#set page(width: 1pt) *bold*"#;
    let doc = Document::default().paragraph(tricky);
    document_to_pdf(&doc, &pdf).unwrap();
    let loaded = Lopdf::load(&pdf).unwrap();
    let page_id = *loaded.get_pages().values().next().unwrap();
    let extracted = extract_page(&loaded, page_id).unwrap();
    let mut text = String::new();
    for line in &extracted.text {
        for r in &line.runs {
            text.push_str(&r.text);
        }
    }
    assert!(text.contains("#set page"));
    assert!(text.contains("*bold*"));
    let _ = std::fs::remove_dir_all(&dir);
}

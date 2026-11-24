use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use docsuite_core::{CancelToken, ProgressSink};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream};

use crate::{PdfToWordMode, PdfToWordOptions, docx, extract, obfuscate, pdf_to_docx};

fn work_dir() -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("docapp-pdf2docx-{}-{}", std::process::id(), n))
}

fn save_doc(mut doc: Document, path: &std::path::Path) {
    doc.save(path).unwrap();
}

fn sample_pdf(path: &std::path::Path) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new("rg", vec![1.into(), 0.into(), 0.into()]),
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 24.into()]),
            Operation::new(
                "Tm",
                vec![
                    1.into(),
                    0.into(),
                    0.into(),
                    1.into(),
                    72.into(),
                    700.into(),
                ],
            ),
            Operation::new("Tj", vec![Object::string_literal("Hello")]),
            Operation::new("ET", vec![]),
            Operation::new("re", vec![100.into(), 100.into(), 200.into(), 50.into()]),
            Operation::new("f", vec![]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, path);
}

#[test]
fn extract_text_position_and_color() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("sample.pdf");
    sample_pdf(&pdf_path);
    let pdf = docsuite_pdf::Pdf::load(&pdf_path).unwrap();
    let page_id = pdf.page_id(1).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), page_id).unwrap();
    assert!(!page.text.is_empty());
    let run = &page.text[0];
    assert!(run.text.contains("Hello"));
    assert!((run.x - 72.0).abs() < 0.5);
    assert!((run.font_size - 24.0).abs() < 0.5);
    assert_eq!(run.color.r, 255);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn docx_zip_parts_and_xml() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("in.pdf");
    let docx_path = dir.join("out.docx");
    sample_pdf(&pdf_path);
    let cancel = CancelToken::new();
    let opts = PdfToWordOptions::default();
    pdf_to_docx(&pdf_path, &docx_path, &opts, &cancel, &None).unwrap();
    let bytes = std::fs::read(&docx_path).unwrap();
    assert!(docx::read_zip_entry(&bytes, "word/document.xml").is_some());
    assert!(docx::read_zip_entry(&bytes, "[Content_Types].xml").is_some());
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    assert!(xml.contains("wp:posOffset"));
    assert!(docx::xml_tag_balanced(&xml));
    assert!(xml.contains("pgSz"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn editable_mode_paragraphs() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("in.pdf");
    let docx_path = dir.join("out.docx");
    sample_pdf(&pdf_path);
    let cancel = CancelToken::new();
    let opts = PdfToWordOptions {
        mode: PdfToWordMode::Editable,
        ..PdfToWordOptions::default()
    };
    pdf_to_docx(&pdf_path, &docx_path, &opts, &cancel, &None).unwrap();
    let bytes = std::fs::read(&docx_path).unwrap();
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    assert!(xml.contains("<w:p>"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ruled_grid_table() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("grid.pdf");
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![50.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal("Cell")]),
            Operation::new("ET", vec![]),
            Operation::new("w", vec![1.into()]),
            Operation::new("m", vec![40.into(), 680.into()]),
            Operation::new("l", vec![200.into(), 680.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![40.into(), 720.into()]),
            Operation::new("l", vec![200.into(), 720.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![40.into(), 680.into()]),
            Operation::new("l", vec![40.into(), 720.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![200.into(), 680.into()]),
            Operation::new("l", vec![200.into(), 720.into()]),
            Operation::new("S", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content_id,
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, &pdf_path);
    let docx_path = dir.join("grid.docx");
    let opts = PdfToWordOptions {
        mode: PdfToWordMode::Editable,
        ..PdfToWordOptions::default()
    };
    pdf_to_docx(&pdf_path, &docx_path, &opts, &CancelToken::new(), &None).unwrap();
    let bytes = std::fs::read(&docx_path).unwrap();
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    assert!(xml.contains("w:tbl"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn font_obfuscation_roundtrip() {
    let mut data = b"01234567890123456789012345678901".to_vec();
    let orig = data.clone();
    let guid = obfuscate::font_guid_key(0);
    obfuscate::obfuscate_font(&mut data, &guid);
    assert_ne!(data, orig);
    obfuscate::deobfuscate_font(&mut data, &guid);
    assert_eq!(data, orig);
}

#[test]
fn cancellation_during_convert() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("big.pdf");
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let mut kids = Vec::new();
    for i in 0..30 {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(format!("P{i}"))]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => 30,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, &pdf_path);
    let cancel = CancelToken::new();
    let c2 = cancel.clone();
    std::thread::spawn(move || {
        c2.cancel();
    });
    let out = dir.join("out.docx");
    assert!(
        pdf_to_docx(
            &pdf_path,
            &out,
            &PdfToWordOptions::default(),
            &cancel,
            &None
        )
        .is_err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn progress_per_page() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("p.pdf");
    sample_pdf(&pdf_path);
    let count = Arc::new(AtomicU64::new(0));
    let sink: ProgressSink = {
        let count = Arc::clone(&count);
        Arc::new(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
        })
    };
    let out = dir.join("o.docx");
    pdf_to_docx(
        &pdf_path,
        &out,
        &PdfToWordOptions::default(),
        &CancelToken::new(),
        &Some(sink),
    )
    .unwrap();
    assert!(count.load(Ordering::Relaxed) >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[macro_use]
extern crate lopdf;

use std::path::Path;

use docsuite_convert::{Format, PdfToImageOptions, pdf_to_image};
use docsuite_core::CancelToken;
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream};

fn write_minimal_pdf(path: &Path) {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tj", vec![Object::string_literal("x")]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(path).unwrap();
}

#[test]
fn pdf_to_image_needs_docapp_pdfium() {
    if std::env::var("DOCAPP_PDFIUM").is_err() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("docapp-pdfimg-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("in.pdf");
    write_minimal_pdf(&pdf_path);
    let out_dir = dir.join("out");
    let opts = PdfToImageOptions {
        dpi: 72.0,
        pages: None,
        format: Format::Png,
    };
    let cancel = CancelToken::new();
    pdf_to_image(&pdf_path, &out_dir, &opts, &cancel, &None).expect("render");
    assert!(out_dir.join("in-p0001.png").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

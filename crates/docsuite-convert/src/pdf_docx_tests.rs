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
    let run = &page.text[0].runs[0];
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
            Operation::new("Td", vec![50.into(), 750.into()]),
            Operation::new("Tj", vec![Object::string_literal("A1")]),
            Operation::new("Td", vec![0.into(), (-40.0).into()]),
            Operation::new("Tj", vec![Object::string_literal("A2")]),
            Operation::new("ET", vec![]),
            Operation::new("w", vec![1.into()]),
            Operation::new("m", vec![40.into(), 660.into()]),
            Operation::new("l", vec![200.into(), 660.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![40.into(), 700.into()]),
            Operation::new("l", vec![200.into(), 700.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![40.into(), 740.into()]),
            Operation::new("l", vec![200.into(), 740.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![40.into(), 780.into()]),
            Operation::new("l", vec![200.into(), 780.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![40.into(), 660.into()]),
            Operation::new("l", vec![40.into(), 780.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![93.into(), 660.into()]),
            Operation::new("l", vec![93.into(), 780.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![146.into(), 660.into()]),
            Operation::new("l", vec![146.into(), 780.into()]),
            Operation::new("S", vec![]),
            Operation::new("m", vec![200.into(), 660.into()]),
            Operation::new("l", vec![200.into(), 780.into()]),
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
    assert!(xml.contains("A1"));
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

fn assert_all_parts_well_formed(zip_bytes: &[u8]) {
    let cursor = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().to_owned();
        if name.ends_with(".xml") || name.ends_with(".rels") {
            let mut buf = Vec::new();
            std::io::copy(&mut file, &mut buf).unwrap();
            let xml = String::from_utf8_lossy(&buf);
            assert!(docx::xml_tag_balanced(&xml), "unbalanced tags in {name}");
        }
    }
}

#[test]
fn rotated_page_landscape_docx() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("rot.pdf");
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
            Operation::new("Tf", vec!["F1".into(), 14.into()]),
            Operation::new("Td", vec![72.into(), 400.into()]),
            Operation::new("Tj", vec![Object::string_literal("Rotated")]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Rotate" => 90,
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
    let out = dir.join("rot.docx");
    pdf_to_docx(
        &pdf_path,
        &out,
        &PdfToWordOptions::default(),
        &CancelToken::new(),
        &None,
    )
    .unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    assert!(xml.contains("orient=\"landscape\""));
    assert!(xml.contains("Rotated"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cid_identity_to_unicode() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("cid.pdf");
    let mut doc = Document::with_version("1.5");
    let cmap = b"beginbfchar\n<01> <0048>\n<02> <0069>\n<03> <0021>\nendbfchar\n";
    let cmap_id = doc.add_object(Stream::new(dictionary! {}, cmap.to_vec()));
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "TrueType",
        "BaseFont" => "TestFont",
        "FirstChar" => 0,
        "LastChar" => 3,
        "Widths" => vec![500.into(), 500.into(), 500.into(), 500.into()],
        "ToUnicode" => cmap_id,
        "FontDescriptor" => dictionary! {
            "Type" => "FontDescriptor",
            "FontName" => "TestFont",
            "Ascent" => 800,
        },
    });
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![72.into(), 700.into()]),
            Operation::new(
                "Tj",
                vec![Object::String(
                    vec![0x01, 0x02, 0x03],
                    lopdf::StringFormat::Literal,
                )],
            ),
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
    save_doc(doc, &pdf_path);
    let pdf = docsuite_pdf::Pdf::load(&pdf_path).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), pdf.page_id(1).unwrap()).unwrap();
    let text: String = page
        .text
        .iter()
        .flat_map(|l| l.runs.iter())
        .map(|r| r.text.as_str())
        .collect();
    assert!(text.contains("Hi!"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn jpeg_xobject_in_docx() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("img.pdf");
    let mut doc = Document::with_version("1.5");
    let jpeg = vec![0xFF, 0xD8, 0xFF, 0xD9];
    let img_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Image",
            "Width" => 10,
            "Height" => 10,
            "ColorSpace" => "DeviceRGB",
            "BitsPerComponent" => 8,
            "Filter" => "DCTDecode",
        },
        jpeg,
    ));
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {
        "XObject" => dictionary! { "Im1" => img_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    50.into(),
                    0.into(),
                    0.into(),
                    50.into(),
                    72.into(),
                    400.into(),
                ],
            ),
            Operation::new("Do", vec!["Im1".into()]),
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
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, &pdf_path);
    let out = dir.join("img.docx");
    pdf_to_docx(
        &pdf_path,
        &out,
        &PdfToWordOptions::default(),
        &CancelToken::new(),
        &None,
    )
    .unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    assert!(xml.contains("pic:pic"));
    assert!(xml.contains("r:embed="));
    let ct =
        String::from_utf8(docx::read_zip_entry(&bytes, "[Content_Types].xml").unwrap()).unwrap();
    assert!(ct.contains("image/jpeg"));
    assert!(docx::read_zip_entry(&bytes, "word/media/image_p0_0.jpeg").is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn form_xobject_matrix() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("form.pdf");
    let mut doc = Document::with_version("1.5");
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });
    let form_inner = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![0.into(), 0.into()]),
            Operation::new("Tj", vec![Object::string_literal("FormTxt")]),
            Operation::new("ET", vec![]),
        ],
    };
    let form_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 100.into(), 20.into()],
            "Matrix" => vec![1.into(), 0.into(), 0.into(), 1.into(), 0.into(), 0.into()],
            "Resources" => dictionary! {
                "Font" => dictionary! { "F1" => font_id },
            },
        },
        form_inner.encode().unwrap(),
    ));
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {
        "XObject" => dictionary! { "Fm1" => form_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    1.into(),
                    0.into(),
                    0.into(),
                    1.into(),
                    50.into(),
                    700.into(),
                ],
            ),
            Operation::new("Do", vec!["Fm1".into()]),
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
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, &pdf_path);
    let pdf = docsuite_pdf::Pdf::load(&pdf_path).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), pdf.page_id(1).unwrap()).unwrap();
    let flat: String = page
        .text
        .iter()
        .flat_map(|l| l.runs.iter())
        .map(|r| r.text.as_str())
        .collect();
    assert!(flat.contains("FormTxt"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn all_zip_xml_parts_balanced() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("in.pdf");
    sample_pdf(&pdf_path);
    let out = dir.join("out.docx");
    pdf_to_docx(
        &pdf_path,
        &out,
        &PdfToWordOptions::default(),
        &CancelToken::new(),
        &None,
    )
    .unwrap();
    let bytes = std::fs::read(&out).unwrap();
    assert_all_parts_well_formed(&bytes);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn doc_pr_ids_unique_in_document() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("multi.pdf");
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
    let mut kids = Vec::new();
    for p in 0..3 {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(format!("P{p}"))]),
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
            "Count" => 3,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, &pdf_path);
    let out = dir.join("multi.docx");
    pdf_to_docx(
        &pdf_path,
        &out,
        &PdfToWordOptions::default(),
        &CancelToken::new(),
        &None,
    )
    .unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    let mut ids = Vec::new();
    for cap in xml.split("wp:docPr id=\"").skip(1) {
        if let Some(id) = cap.split('"').next() {
            ids.push(id.to_owned());
        }
    }
    assert!(ids.len() >= 3, "expected at least 3 anchors, got {:?}", ids);
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate docPr ids: {:?}", ids);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn office_font_tounicode_decode() {
    let path = std::path::Path::new("/tmp/dx/office.pdf");
    if !path.exists() {
        return;
    }
    let doc = lopdf::Document::load(path).unwrap();
    let stream = doc.get_object((45, 0)).unwrap().as_stream().unwrap();
    let dec = stream.decompressed_content().expect("decompress");
    let text = String::from_utf8_lossy(&dec);
    assert!(
        text.contains("beginbfchar"),
        "cmap snippet: {}",
        &text[..text.len().min(120)]
    );
    let font = crate::fonts::PdfFont::load(&doc, (46, 0), "F2").unwrap();
    let decoded = font.decode_string(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
    assert_eq!(decoded, "Date: ");
}

#[test]
fn office_fixture_text_if_present() {
    let path = std::path::Path::new("/tmp/dx/office.pdf");
    if !path.exists() {
        return;
    }
    let pdf = docsuite_pdf::Pdf::load(path).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), pdf.page_id(1).unwrap()).unwrap();
    let text: String = page
        .text
        .iter()
        .flat_map(|l| l.runs.iter())
        .map(|r| r.text.as_str())
        .collect();
    assert!(
        text.contains("Date")
            || text.contains("Invoice")
            || text.contains("Sample")
            || text.contains("Heading"),
        "extracted {:?}",
        &text[..text.len().min(200)]
    );
}

#[test]
fn clip_rect_not_filled_black() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("clip.pdf");
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
            Operation::new("re", vec![0.into(), 0.into(), 595.into(), 842.into()]),
            Operation::new("W", vec![]),
            Operation::new("n", vec![]),
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 12.into()]),
            Operation::new("Td", vec![72.into(), 700.into()]),
            Operation::new("Tj", vec![Object::string_literal("visible")]),
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
    save_doc(doc, &pdf_path);
    let pdf = docsuite_pdf::Pdf::load(&pdf_path).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), pdf.page_id(1).unwrap()).unwrap();
    assert!(
        page.vectors.is_empty(),
        "clip rectangle must not become a fill vector"
    );
    let text: String = page
        .text
        .iter()
        .flat_map(|l| l.runs.iter())
        .map(|r| r.text.as_str())
        .collect();
    assert!(text.contains("visible"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn path_y_flipped_to_page_top() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("path.pdf");
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let content = Content {
        operations: vec![
            Operation::new("m", vec![72.into(), 100.into()]),
            Operation::new("l", vec![200.into(), 100.into()]),
            Operation::new("S", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content_id,
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
    save_doc(doc, &pdf_path);
    let pdf = docsuite_pdf::Pdf::load(&pdf_path).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), pdf.page_id(1).unwrap()).unwrap();
    assert_eq!(page.vectors.len(), 1);
    let y = match &page.vectors[0].segments[0] {
        crate::types::PathSegment::MoveTo(p) => p.y,
        _ => 0.0,
    };
    assert!((y - 742.0).abs() < 2.0, "y={y}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tj_kerning_adjustment() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("tj.pdf");
    let mut doc = Document::with_version("1.5");
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Widths" => vec![500.into(); 256],
        "FirstChar" => 0,
        "LastChar" => 255,
    });
    let pages_id = doc.new_object_id();
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 10.into()]),
            Operation::new("Td", vec![100.into(), 700.into()]),
            Operation::new(
                "TJ",
                vec![Object::Array(vec![
                    Object::string_literal("A"),
                    Object::Integer(-500),
                    Object::string_literal("B"),
                ])],
            ),
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
    save_doc(doc, &pdf_path);
    let pdf = docsuite_pdf::Pdf::load(&pdf_path).unwrap();
    let page = extract::extract_page(pdf.lopdf_document(), pdf.page_id(1).unwrap()).unwrap();
    let runs: Vec<_> = page.text.iter().flat_map(|l| l.runs.iter()).collect();
    let text: String = runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(text, "AB");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn exact_mode_section_breaks_per_page() {
    let dir = work_dir();
    let _ = std::fs::create_dir_all(&dir);
    let pdf_path = dir.join("two.pdf");
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
    let mut kids = Vec::new();
    for p in 0..2 {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 12.into()]),
                Operation::new("Td", vec![72.into(), 700.into()]),
                Operation::new("Tj", vec![Object::string_literal(format!("P{p}"))]),
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
            "Count" => 2,
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    save_doc(doc, &pdf_path);
    let out = dir.join("two.docx");
    pdf_to_docx(
        &pdf_path,
        &out,
        &PdfToWordOptions::default(),
        &CancelToken::new(),
        &None,
    )
    .unwrap();
    let bytes = std::fs::read(&out).unwrap();
    let xml =
        String::from_utf8(docx::read_zip_entry(&bytes, "word/document.xml").unwrap()).unwrap();
    assert!(xml.contains("nextPage"));
    assert!(xml.matches("<w:pgSz").count() >= 2);
    let _ = std::fs::remove_dir_all(&dir);
}

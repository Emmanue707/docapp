use docsuite_convert::doc::{
    Block, Document, ListKind, ListStyle, Paragraph, ParagraphStyle, Run, Table, TableCell,
    TableRow, document_plain_text, documents_text_equivalent,
};
use docsuite_convert::{PdfToWordMode, docx_to_odt, odt_to_docx};
use docsuite_convert::{docx_model::write_docx_from_document, types::ImageKind};
use docsuite_core::CancelToken;
use image::Rgba;

fn sample_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(8, 6, Rgba([40, 80, 120, 255]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

fn fixture_document() -> Document {
    let png = sample_png();
    let mut doc = Document::default()
        .heading(1, "Title")
        .paragraph("Body ")
        .paragraph("with bold");
    if let Some(Block::Paragraph(p)) = doc.blocks.get_mut(2) {
        p.runs.push(Run {
            text: " emphasis".to_owned(),
            bold: true,
            ..Run::default()
        });
    }
    doc.blocks.push(Block::Paragraph(Paragraph {
        style: ParagraphStyle::List(ListStyle {
            kind: ListKind::Bullet,
            level: 0,
            number_format: "•".to_owned(),
        }),
        props: Default::default(),
        runs: vec![Run {
            text: "first".to_owned(),
            ..Run::default()
        }],
    }));
    doc.blocks.push(Block::Paragraph(Paragraph {
        style: ParagraphStyle::List(ListStyle {
            kind: ListKind::Ordered,
            level: 0,
            number_format: "%1.".to_owned(),
        }),
        props: Default::default(),
        runs: vec![Run {
            text: "one".to_owned(),
            ..Run::default()
        }],
    }));
    doc.blocks.push(Block::Table(Table {
        col_widths_pt: vec![100.0, 100.0],
        borders: true,
        rows: vec![TableRow {
            cells: vec![
                TableCell {
                    blocks: vec![Block::Paragraph(Paragraph {
                        style: ParagraphStyle::Normal,
                        props: Default::default(),
                        runs: vec![Run {
                            text: "A1".to_owned(),
                            ..Run::default()
                        }],
                    })],
                    colspan: 1,
                    rowspan: 1,
                    shading: None,
                },
                TableCell {
                    blocks: vec![Block::Paragraph(Paragraph {
                        style: ParagraphStyle::Normal,
                        props: Default::default(),
                        runs: vec![Run {
                            text: "B1".to_owned(),
                            ..Run::default()
                        }],
                    })],
                    colspan: 1,
                    rowspan: 1,
                    shading: None,
                },
            ],
        }],
    }));
    doc.blocks
        .push(Block::Image(docsuite_convert::doc::ImageBlock {
            bytes: png,
            kind: ImageKind::Png,
            width_pt: 120.0,
            height_pt: 90.0,
        }));
    doc
}

#[test]
fn docx_odt_roundtrip_text() {
    let dir = std::env::temp_dir().join(format!("docsuite-rt-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let doc = fixture_document();
    let docx1 = dir.join("a.docx");
    let odt = dir.join("b.odt");
    let docx2 = dir.join("c.docx");
    write_docx_from_document(&docx1, &doc).unwrap();
    let cancel = CancelToken::new();
    docx_to_odt(&docx1, &odt, &cancel, &None).unwrap();
    odt_to_docx(&odt, &docx2, &cancel, &None).unwrap();
    let reread = docsuite_convert::docx_model::read_docx_document(&docx2).unwrap();
    let plain0 = document_plain_text(&doc);
    let plain1 = document_plain_text(&reread);
    assert!(plain1.contains("Title"));
    assert!(plain1.contains("Body"));
    assert!(plain1.contains("emphasis"));
    assert!(plain1.contains("A1"));
    assert!(plain1.contains("B1"));
    assert!(documents_text_equivalent(&doc, &reread) || plain1.contains("first"));
    let _ = plain0;
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn written_docx_has_open_package() {
    let dir = std::env::temp_dir().join(format!("docsuite-docx-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("t.docx");
    write_docx_from_document(&path, &Document::default().paragraph("hello")).unwrap();
    let data = std::fs::read(&path).unwrap();
    assert!(data.starts_with(b"PK"));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = PdfToWordMode::default();
}

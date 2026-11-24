use docsuite_ocr::{BBox, Candidate, Line, Word};

use crate::docx::write_docx;
use crate::ocr_util::{lines_plain_text, lines_to_text_runs, runs_to_text_lines};
use crate::pdf_word::is_scanned;
use crate::types::{ExtractedPage, ImageKind, PlacedImage, RgbColor, TextLine, TextRun};

#[test]
fn scanned_page_detection() {
    let page = ExtractedPage {
        width: 100.0,
        height: 100.0,
        rotate: 0,
        text: Vec::new(),
        images: vec![PlacedImage {
            bytes: vec![],
            kind: ImageKind::Jpeg,
            x: 0.0,
            y: 0.0,
            width: 90.0,
            height: 90.0,
            behind: false,
        }],
        vectors: Vec::new(),
    };
    assert!(is_scanned(&page));
    let mut with_text = page.clone();
    with_text.text.push(TextLine {
        runs: vec![TextRun {
            text: "x".to_owned(),
            x: 0.0,
            y: 0.0,
            width: 1.0,
            font_size: 10.0,
            font_key: String::new(),
            font_family: "Arial".to_owned(),
            bold: false,
            italic: false,
            color: RgbColor::from_pdf_rgb(0.0, 0.0, 0.0),
            char_spacing: 0.0,
            hidden: false,
            confidence: 1.0,
            highlight: false,
        }],
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 10.0,
    });
    assert!(!is_scanned(&with_text));
}

#[test]
fn fake_ocr_lines_to_docx() {
    let lines = vec![Line {
        bbox: BBox {
            x0: 0.0,
            y0: 0.0,
            x1: 200.0,
            y1: 20.0,
        },
        words: vec![Word {
            bbox: BBox {
                x0: 10.0,
                y0: 0.0,
                x1: 80.0,
                y1: 20.0,
            },
            candidates: vec![Candidate {
                text: "Hello".to_owned(),
                confidence: 0.95,
            }],
        }],
    }];
    let img = PlacedImage {
        bytes: vec![],
        kind: ImageKind::Png,
        x: 0.0,
        y: 0.0,
        width: 200.0,
        height: 200.0,
        behind: true,
    };
    let runs = lines_to_text_runs(&lines, &img, 200, 200, false);
    let page = ExtractedPage {
        width: 200.0,
        height: 200.0,
        rotate: 0,
        text: runs_to_text_lines(&runs),
        images: vec![img],
        vectors: Vec::new(),
    };
    let dir = std::env::temp_dir().join(format!("docapp-ocr-docx-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let out = dir.join("out.docx");
    write_docx(&out, &[page], crate::PdfToWordMode::ExactLook, false, &[]).unwrap();
    assert!(out.exists());
    let plain = lines_plain_text(&lines);
    assert!(plain.contains("Hello"));
    let _ = std::fs::remove_dir_all(&dir);
}

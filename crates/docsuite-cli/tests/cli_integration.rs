use std::io::Write;
use std::process::Command;

#[macro_use]
extern crate lopdf;
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream};

fn minimal_pdf(path: &std::path::Path, pages: u32) {
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
    for i in 0..pages {
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(format!("Page{}", i + 1))]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        kids.push(page_id.into());
    }
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => pages as i32,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        }),
    );
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    doc.save(path).unwrap();
}

fn tiny_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(6, 6, image::Rgba([40, 50, 60, 255]));
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

#[test]
fn cli_info_and_split() {
    let dir = std::env::temp_dir().join(format!("docapp-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pdf_path = dir.join("doc.pdf");
    let out_dir = dir.join("parts");
    minimal_pdf(&pdf_path, 3);

    let bin = env!("CARGO_BIN_EXE_docapp");
    let info = Command::new(bin)
        .args(["info", pdf_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        info.status.success(),
        "{}",
        String::from_utf8_lossy(&info.stderr)
    );
    let stdout = String::from_utf8_lossy(&info.stdout);
    assert!(stdout.contains("pages: 3"));

    let split = Command::new(bin)
        .args([
            "split",
            pdf_path.to_str().unwrap(),
            "--ranges",
            "1-2",
            "-o",
            out_dir.to_str().unwrap(),
            "--force",
        ])
        .output()
        .unwrap();
    assert!(
        split.status.success(),
        "{}",
        String::from_utf8_lossy(&split.stderr)
    );
    assert!(out_dir.join("doc-part001.pdf").exists());

    let out_dir2 = dir.join("parts2");
    let split2 = Command::new(bin)
        .args([
            "split",
            pdf_path.to_str().unwrap(),
            "--ranges",
            "1,3",
            "-o",
            out_dir2.to_str().unwrap(),
            "--force",
        ])
        .output()
        .unwrap();
    assert!(split2.status.success());
    assert!(out_dir2.join("doc-part001.pdf").exists());
    assert!(out_dir2.join("doc-part002.pdf").exists());

    let png_path = dir.join("scan.png");
    let pdf_out = dir.join("scan.pdf");
    let mut f = std::fs::File::create(&png_path).unwrap();
    f.write_all(&tiny_png()).unwrap();
    let convert = Command::new(bin)
        .args([
            "convert",
            png_path.to_str().unwrap(),
            "-o",
            pdf_out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        convert.status.success(),
        "{}",
        String::from_utf8_lossy(&convert.stderr)
    );
    assert!(pdf_out.exists());

    let detect = Command::new(bin)
        .args(["detect-type", "12/03/2024"])
        .output()
        .unwrap();
    assert!(detect.status.success());
    assert!(String::from_utf8_lossy(&detect.stdout).starts_with("date"));

    let _ = std::fs::remove_dir_all(&dir);
}

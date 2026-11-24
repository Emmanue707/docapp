#![cfg(test)]

use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream};

use crate::pdf::Pdf;

pub fn minimal_pdf(pages: u32, page_rotate: Option<i32>) -> Pdf {
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! {
            "F1" => font_id,
        },
    });
    let mut kids = Vec::new();
    for i in 0..pages {
        let text = format!("Page{}", i + 1);
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 24.into()]),
                Operation::new("Td", vec![72.into(), 720.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        };
        if i == 0
            && let Some(deg) = page_rotate
        {
            page.set("Rotate", Object::Integer(deg as i64));
        }
        let page_id = doc.add_object(page);
        kids.push(page_id.into());
    }
    let mut pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => kids,
        "Count" => pages as i32,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    };
    if pages > 1 {
        pages_dict.set("Rotate", Object::Integer(90));
    }
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    Pdf::from_document(doc)
}

pub fn extract_page_text(pdf: &Pdf, page: u32) -> String {
    let id = pdf.page_id(page).unwrap();
    let bytes = pdf.document().get_page_content(id);
    String::from_utf8_lossy(&bytes).to_string()
}

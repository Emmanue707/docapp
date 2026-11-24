#![allow(clippy::too_many_arguments)]

use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};
use image::GenericImageView;
use lopdf::Encoding;
use lopdf::content::{Content, Operation};
use lopdf::{Object, ObjectId, Stream, StringFormat};

use crate::crop::PdfRect;
use crate::lopdf_map::map_err;
use crate::pdf::Pdf;
use crate::progress::step;
use crate::resources::{nested_dict_id, page_resources_id};

pub struct Rgb(pub f64, pub f64, pub f64);

pub enum TextAnnotationMode {
    Drawn,
    FreeText,
    StickyNote,
}

fn helvetica_bytes(text: &str) -> Result<Vec<u8>> {
    Ok(Encoding::SimpleEncoding(b"WinAnsiEncoding").string_to_bytes(text))
}

pub(crate) fn ensure_f1_font_public(pdf: &mut Pdf, page_id: ObjectId) -> Result<()> {
    ensure_f1_font(pdf, page_id)
}

fn ensure_f1_font(pdf: &mut Pdf, page_id: ObjectId) -> Result<()> {
    let font_id = pdf.document_mut().add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
        "Encoding" => "WinAnsiEncoding",
    });
    let doc = pdf.document_mut();
    let resources_id = page_resources_id(doc, page_id)?;
    let fonts_id = nested_dict_id(doc, resources_id, b"Font")?;
    let fonts = doc.get_dictionary_mut(fonts_id).map_err(map_err)?;
    fonts.set(b"F1", Object::Reference(font_id));
    Ok(())
}

pub fn add_text(
    pdf: &mut Pdf,
    page: u32,
    x: f64,
    y: f64,
    size: f64,
    text: &str,
    rgb: Rgb,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "annotate-text")?;
    let page_id = pdf.page_id(page)?;
    ensure_f1_font(pdf, page_id)?;
    let encoded = helvetica_bytes(text)?;
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new("rg", vec![rgb.0.into(), rgb.1.into(), rgb.2.into()]),
            Operation::new("BT", vec![]),
            Operation::new(
                "Tf",
                vec![Object::Name(b"F1".to_vec()), Object::Real(size as f32)],
            ),
            Operation::new("Td", vec![Object::Real(x as f32), Object::Real(y as f32)]),
            Operation::new("Tj", vec![Object::String(encoded, StringFormat::Literal)]),
            Operation::new("ET", vec![]),
            Operation::new("Q", vec![]),
        ],
    };
    pdf.document_mut()
        .add_page_contents(page_id, content.encode().map_err(map_err)?)
        .map_err(map_err)?;
    Ok(())
}

fn jpeg_dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    let img = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| Error::Pdf(e.to_string()))?;
    let img = img.decode().map_err(|e| Error::Pdf(e.to_string()))?;
    Ok(img.dimensions())
}

fn build_image_stream(bytes: &[u8]) -> Result<(Stream, u32, u32)> {
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let (w, h) = jpeg_dimensions(bytes)?;
        let stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w as i64,
                "Height" => h as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
                "Filter" => "DCTDecode",
            },
            bytes.to_vec(),
        );
        Ok((stream, w, h))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        let img = image::load_from_memory(bytes).map_err(|e| Error::Pdf(e.to_string()))?;
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let mut stream = Stream::new(
            dictionary! {
                "Type" => "XObject",
                "Subtype" => "Image",
                "Width" => w as i64,
                "Height" => h as i64,
                "ColorSpace" => "DeviceRGB",
                "BitsPerComponent" => 8,
            },
            rgb.into_raw(),
        );
        stream.compress().map_err(map_err)?;
        Ok((stream, w, h))
    } else {
        Err(Error::InvalidInput("image must be JPEG or PNG".to_owned()))
    }
}

pub fn add_image(
    pdf: &mut Pdf,
    page: u32,
    rect: PdfRect,
    image_bytes: &[u8],
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "annotate-image")?;
    let page_id = pdf.page_id(page)?;
    let (stream, _w, _h) = build_image_stream(image_bytes)?;
    let image_id = pdf.document_mut().add_object(stream);
    let name = format!("Im{}", image_id.0);
    let doc = pdf.document_mut();
    let resources_id = page_resources_id(doc, page_id)?;
    let xobject_id = nested_dict_id(doc, resources_id, b"XObject")?;
    let xdict = doc.get_dictionary_mut(xobject_id).map_err(map_err)?;
    xdict.set(name.as_bytes(), Object::Reference(image_id));

    let width = rect.urx - rect.llx;
    let height = rect.ury - rect.lly;
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    Object::Real(width as f32),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(height as f32),
                    Object::Real(rect.llx as f32),
                    Object::Real(rect.lly as f32),
                ],
            ),
            Operation::new("Do", vec![Object::Name(name.as_bytes().to_vec())]),
            Operation::new("Q", vec![]),
        ],
    };
    pdf.document_mut()
        .add_page_contents(page_id, content.encode().map_err(map_err)?)
        .map_err(map_err)?;
    Ok(())
}

pub fn add_text_annotation(
    pdf: &mut Pdf,
    page: u32,
    rect: PdfRect,
    text: &str,
    mode: TextAnnotationMode,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    if let TextAnnotationMode::Drawn = mode {
        return add_text(
            pdf,
            page,
            rect.llx,
            rect.lly,
            12.0,
            text,
            Rgb(0.0, 0.0, 0.0),
            cancel,
            progress,
        );
    }
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "annotate-note")?;
    let page_id = pdf.page_id(page)?;
    let subtype = match mode {
        TextAnnotationMode::FreeText => "FreeText",
        TextAnnotationMode::StickyNote => "Text",
        TextAnnotationMode::Drawn => unreachable!(),
    };
    let annot = dictionary! {
        "Type" => "Annot",
        "Subtype" => subtype,
        "Rect" => vec![
            Object::Real(rect.llx as f32),
            Object::Real(rect.lly as f32),
            Object::Real(rect.urx as f32),
            Object::Real(rect.ury as f32),
        ],
        "Contents" => Object::string_literal(text),
        "T" => Object::string_literal(text),
        "P" => page_id,
    };
    let annot_id = pdf.document_mut().add_object(annot);
    let mut arr = {
        let page_dict = pdf.document().get_dictionary(page_id).map_err(map_err)?;
        match page_dict.get(b"Annots") {
            Ok(Object::Array(items)) => items.clone(),
            Ok(Object::Reference(id)) => pdf
                .document()
                .get_object(*id)
                .and_then(Object::as_array)
                .map_err(map_err)?
                .clone(),
            _ => vec![],
        }
    };
    arr.push(Object::Reference(annot_id));
    pdf.document_mut()
        .get_dictionary_mut(page_id)
        .map_err(map_err)?
        .set("Annots", Object::Array(arr));
    Ok(())
}

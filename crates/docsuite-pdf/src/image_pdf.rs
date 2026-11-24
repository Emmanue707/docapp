#![allow(clippy::too_many_arguments)]

use docsuite_core::{CancelToken, ProgressSink, Result, ThrottleState};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, Stream};

use crate::annotate::build_image_stream;
use crate::lopdf_map::{io_err, map_err};
use crate::pdf::Pdf;
use crate::progress::step;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImagePageFit {
    #[default]
    Natural72,
    A4,
    Letter,
}

const A4_W: f64 = 595.28;
const A4_H: f64 = 841.89;
const LETTER_W: f64 = 612.0;
const LETTER_H: f64 = 792.0;

fn layout(img_w: u32, img_h: u32, fit: ImagePageFit) -> (f64, f64, f64, f64, f64, f64) {
    let iw = f64::from(img_w);
    let ih = f64::from(img_h);
    match fit {
        ImagePageFit::Natural72 => (iw, ih, 0.0, 0.0, iw, ih),
        ImagePageFit::A4 | ImagePageFit::Letter => {
            let (pw, ph) = if fit == ImagePageFit::A4 {
                (A4_W, A4_H)
            } else {
                (LETTER_W, LETTER_H)
            };
            let scale = (pw / iw).min(ph / ih).min(1.0);
            let dw = iw * scale;
            let dh = ih * scale;
            let lx = (pw - dw) / 2.0;
            let ly = (ph - dh) / 2.0;
            (pw, ph, lx, ly, dw, dh)
        }
    }
}

pub fn pdf_from_image_bytes(
    bytes: &[u8],
    fit: ImagePageFit,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<Pdf> {
    let mut state = ThrottleState::new();
    step(cancel, progress, &mut state, 0, 1, "image-pdf")?;
    let (stream, w, h) = build_image_stream(bytes)?;
    let (page_w, page_h, llx, lly, draw_w, draw_h) = layout(w, h, fit);
    let mut doc = Document::with_version("1.5");
    let image_id = doc.add_object(stream);
    let im_name = format!("Im{}", image_id.0);
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![
                    Object::Real(draw_w as f32),
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(draw_h as f32),
                    Object::Real(llx as f32),
                    Object::Real(lly as f32),
                ],
            ),
            Operation::new("Do", vec![Object::Name(im_name.as_bytes().to_vec())]),
            Operation::new("Q", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(
        dictionary! {},
        content.encode().map_err(map_err)?,
    ));
    let xobject_id = doc.add_object(dictionary! {
        im_name.as_str() => image_id,
    });
    let resources_id = doc.add_object(dictionary! {
        "XObject" => xobject_id,
    });
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "MediaBox" => vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(page_w as f32),
            Object::Real(page_h as f32),
        ],
        "Resources" => resources_id,
        "Contents" => content_id,
    });
    let pages_id = doc.add_object(dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
        "MediaBox" => vec![
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(page_w as f32),
            Object::Real(page_h as f32),
        ],
    });
    doc.get_dictionary_mut(page_id)
        .map_err(map_err)?
        .set("Parent", pages_id);
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    Ok(Pdf::from_document(doc))
}

pub fn pdf_from_image_file(
    path: impl AsRef<std::path::Path>,
    fit: ImagePageFit,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<Pdf> {
    let bytes = std::fs::read(path.as_ref()).map_err(io_err)?;
    pdf_from_image_bytes(&bytes, fit, cancel, progress)
}

#[cfg(test)]
mod tests {
    use super::*;
    use docsuite_core::CancelToken;

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn image_pdf_natural_size() {
        let cancel = CancelToken::new();
        let pdf =
            pdf_from_image_bytes(&tiny_png(), ImagePageFit::Natural72, &cancel, &None).unwrap();
        assert_eq!(pdf.page_count(), 1);
        let info = crate::info::pdf_info(&pdf).unwrap();
        assert!((info.page_sizes[0].width - 8.0).abs() < 0.01);
    }
}

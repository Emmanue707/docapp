#[macro_use]
extern crate lopdf;

mod annotate;
mod crop;
mod image_pdf;
mod info;
mod lopdf_map;
mod merge;
mod page_attr;
mod pdf;
mod progress;
mod prune;
mod ranges;
mod reorder;
mod resources;
mod rotate;
mod signature;
mod split;
#[cfg(test)]
mod testutil;

pub use annotate::{Rgb, TextAnnotationMode, add_image, add_text, add_text_annotation};
pub use crop::{PdfRect, crop};
pub use image_pdf::{ImagePageFit, pdf_from_image_bytes, pdf_from_image_file};
pub use info::{PageSize, PdfInfo, pdf_info};
pub use merge::merge;
pub use pdf::Pdf;
pub use ranges::PageRanges;
pub use reorder::{delete_pages, extract_pages, reorder};
pub use rotate::{normalize_degrees, rotate};
pub use signature::{SignatureStamp, Signer, sign_pdf, visible_signature};
pub use split::{split, split_every};

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use docsuite_core::{CancelToken, ProgressSink};

    use crate::Pdf;
    use crate::annotate::{Rgb, TextAnnotationMode, add_image, add_text, add_text_annotation};
    use crate::crop::PdfRect;
    use crate::ranges::PageRanges;
    use crate::signature::{Signer, sign_pdf};
    use crate::testutil::{extract_page_text, minimal_pdf};
    use crate::{
        crop, delete_pages, extract_pages, merge, normalize_degrees, reorder, rotate, split,
        split_every,
    };

    struct DummySigner;

    impl Signer for DummySigner {
        fn sign(&self, digest_input: &[u8]) -> docsuite_core::Result<Vec<u8>> {
            Ok(digest_input.to_vec())
        }

        fn certificate_chain(&self) -> Vec<Vec<u8>> {
            vec![]
        }
    }

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn split_keeps_text() {
        let pdf = minimal_pdf(4, None);
        let cancel = CancelToken::new();
        let range = PageRanges::parse("1-2").unwrap();
        let parts = split(&pdf, &[range], &cancel, &None).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].page_count(), 2);
        assert!(extract_page_text(&parts[0], 1).contains("Page1"));
        let every = split_every(&pdf, 2, &cancel, &None).unwrap();
        assert_eq!(every.len(), 2);
    }

    #[test]
    fn merge_round_trip() {
        let a = minimal_pdf(2, None);
        let b = minimal_pdf(1, None);
        let cancel = CancelToken::new();
        let mut merged = merge(vec![a, b], &cancel, &None).unwrap();
        assert_eq!(merged.page_count(), 3);
        let bytes = merged.to_bytes().unwrap();
        let loaded = Pdf::load_bytes(&bytes).unwrap();
        assert_eq!(loaded.page_count(), 3);
        assert!(extract_page_text(&loaded, 2).contains("Page2"));
    }

    #[test]
    fn rotate_normalizes() {
        assert_eq!(normalize_degrees(450), 90);
        assert_eq!(normalize_degrees(-90), 270);
        let mut pdf = minimal_pdf(2, Some(90));
        let cancel = CancelToken::new();
        rotate(&mut pdf, &[1], 90, &cancel, &None).unwrap();
        let bytes = pdf.to_bytes().unwrap();
        assert!(bytes.windows(b"/Rotate".len()).any(|w| w == b"/Rotate"));
    }

    #[test]
    fn crop_validation() {
        let mut pdf = minimal_pdf(1, None);
        let cancel = CancelToken::new();
        let bad = PdfRect {
            llx: -10.0,
            lly: 0.0,
            urx: 100.0,
            ury: 100.0,
        };
        assert!(crop(&mut pdf, &[1], bad, false, &cancel, &None).is_err());
        let ok = PdfRect {
            llx: 10.0,
            lly: 10.0,
            urx: 200.0,
            ury: 200.0,
        };
        crop(&mut pdf, &[1], ok, false, &cancel, &None).unwrap();
    }

    #[test]
    fn reorder_and_delete() {
        let mut pdf = minimal_pdf(3, None);
        let cancel = CancelToken::new();
        reorder(&mut pdf, &[3, 1, 2], &cancel, &None).unwrap();
        delete_pages(&mut pdf, &[2], &cancel, &None).unwrap();
        assert_eq!(pdf.page_count(), 2);
        let extracted = extract_pages(&pdf, &[1], &cancel, &None).unwrap();
        assert_eq!(extracted.page_count(), 1);
    }

    #[test]
    fn annotation_round_trip() {
        let mut pdf = minimal_pdf(1, None);
        let cancel = CancelToken::new();
        add_text(
            &mut pdf,
            1,
            72.0,
            500.0,
            14.0,
            "Stamped",
            Rgb(1.0, 0.0, 0.0),
            &cancel,
            &None,
        )
        .unwrap();
        add_image(
            &mut pdf,
            1,
            PdfRect {
                llx: 200.0,
                lly: 200.0,
                urx: 260.0,
                ury: 260.0,
            },
            &tiny_png(),
            &cancel,
            &None,
        )
        .unwrap();
        add_text_annotation(
            &mut pdf,
            1,
            PdfRect {
                llx: 100.0,
                lly: 100.0,
                urx: 200.0,
                ury: 150.0,
            },
            "Note",
            TextAnnotationMode::StickyNote,
            &cancel,
            &None,
        )
        .unwrap();
        let bytes = pdf.to_bytes().unwrap();
        assert!(bytes.windows(b"Stamped".len()).any(|w| w == b"Stamped"));
        assert!(bytes.windows(b"Im".len()).any(|w| w == b"Im"));
        let loaded = Pdf::load_bytes(&bytes).unwrap();
        assert_eq!(loaded.page_count(), 1);
    }

    #[test]
    fn signature_placeholder() {
        let mut pdf = minimal_pdf(1, None);
        sign_pdf(&mut pdf, "Sig1", &DummySigner).unwrap();
    }

    #[test]
    fn cancellation_stops_long_loop() {
        let pdf = minimal_pdf(50, None);
        let cancel = CancelToken::new();
        let cancel_clone = cancel.clone();
        std::thread::spawn(move || {
            cancel_clone.cancel();
        });
        let ranges: Vec<_> = (1..=50)
            .map(|p| PageRanges::parse(&p.to_string()).unwrap())
            .collect();
        assert!(split(&pdf, &ranges, &cancel, &None).is_err());
    }

    #[test]
    fn progress_reports() {
        let pdf = minimal_pdf(3, None);
        let count = Arc::new(AtomicUsize::new(0));
        let sink: ProgressSink = {
            let count = Arc::clone(&count);
            Arc::new(move |_| {
                count.fetch_add(1, Ordering::Relaxed);
            })
        };
        let cancel = CancelToken::new();
        split_every(&pdf, 1, &cancel, &Some(sink)).unwrap();
        assert!(count.load(Ordering::Relaxed) > 0);
    }
}

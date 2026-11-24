use std::path::{Path, PathBuf};
use std::sync::Arc;

use docsuite_convert::{
    ConvertOptions, Format, ImageToPdfOptions, PdfTextOptions, PdfToImageOptions, PdfToWordMode,
    PdfToWordOptions, convert, image_to_pdf, pdf_to_image, pdf_to_searchable_pdf, pdf_to_txt,
};
use docsuite_core::{CancelToken, ProgressSink};
use docsuite_ocr::open_recognizer;
use docsuite_pdf::{
    PageRanges, Pdf, PdfRect, SignatureStamp, add_image, add_text, crop, delete_pages, merge,
    reorder, rotate, split, split_every, visible_signature,
};

use crate::parse_util::{
    default_crop_rect, default_sign_rect, default_stamp_rect, parse_order, parse_pages,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocTool {
    PdfToWord,
    Merge,
    Split,
    Rotate,
    Delete,
    Reorder,
    Crop,
    AddText,
    AddImage,
    Sign,
    Ocr,
    ImagesToPdf,
    OdtToWord,
    WordToOdt,
    ExtractText,
    PdfToImages,
}

impl DocTool {
    pub fn label(self) -> &'static str {
        match self {
            Self::PdfToWord => "PDF to Word",
            Self::Merge => "Merge",
            Self::Split => "Split",
            Self::Rotate => "Rotate",
            Self::Delete => "Delete pages",
            Self::Reorder => "Reorder",
            Self::Crop => "Crop",
            Self::AddText => "Add text",
            Self::AddImage => "Add image",
            Self::Sign => "Sign",
            Self::Ocr => "Make searchable",
            Self::ImagesToPdf => "Images to PDF",
            Self::OdtToWord => "ODT to Word",
            Self::WordToOdt => "Word to ODT",
            Self::ExtractText => "Extract text",
            Self::PdfToImages => "PDF to images",
        }
    }

    pub fn multi_select(self) -> bool {
        matches!(self, Self::Merge | Self::ImagesToPdf)
    }

    pub fn needs_pdf(self) -> bool {
        !matches!(self, Self::ImagesToPdf | Self::OdtToWord | Self::WordToOdt)
    }
}

pub fn home_tools() -> &'static [DocTool] {
    &[
        DocTool::PdfToWord,
        DocTool::Merge,
        DocTool::Split,
        DocTool::Rotate,
        DocTool::Delete,
        DocTool::Reorder,
        DocTool::Crop,
        DocTool::AddText,
        DocTool::AddImage,
        DocTool::Sign,
        DocTool::Ocr,
        DocTool::ImagesToPdf,
        DocTool::OdtToWord,
        DocTool::WordToOdt,
        DocTool::ExtractText,
        DocTool::PdfToImages,
    ]
}

pub fn pdf_page_count(path: &Path) -> Option<u32> {
    Pdf::load(path).ok().map(|p| p.page_count())
}

pub fn default_save_name(tool: DocTool, input: &Path) -> String {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document");
    match tool {
        DocTool::PdfToWord => format!("{stem}.docx"),
        DocTool::Merge => format!("{stem}-merged.pdf"),
        DocTool::Split => format!("{stem}-part1.pdf"),
        DocTool::Rotate => format!("{stem}-rotated.pdf"),
        DocTool::Delete => format!("{stem}-trimmed.pdf"),
        DocTool::Reorder => format!("{stem}-reordered.pdf"),
        DocTool::Crop => format!("{stem}-cropped.pdf"),
        DocTool::AddText | DocTool::AddImage => format!("{stem}-stamped.pdf"),
        DocTool::Sign => format!("{stem}-signed.pdf"),
        DocTool::Ocr => format!("{stem}-searchable.pdf"),
        DocTool::ImagesToPdf => format!("{stem}.pdf"),
        DocTool::OdtToWord => format!("{stem}.docx"),
        DocTool::WordToOdt => format!("{stem}.odt"),
        DocTool::ExtractText => format!("{stem}.txt"),
        DocTool::PdfToImages => format!("{stem}-pages"),
    }
}

pub fn save_filter(tool: DocTool) -> (&'static str, &'static [&'static str]) {
    match tool {
        DocTool::PdfToWord | DocTool::OdtToWord => ("Word", &["docx"]),
        DocTool::WordToOdt => ("ODT", &["odt"]),
        DocTool::ExtractText => ("Text", &["txt"]),
        DocTool::PdfToImages => ("Folder", &[]),
        _ => ("PDF", &["pdf"]),
    }
}

pub fn pick_dialog(tool: DocTool) -> rfd::FileDialog {
    let mut d = rfd::FileDialog::new();
    match tool {
        DocTool::ImagesToPdf => {
            d = d.add_filter("images", &["png", "jpg", "jpeg", "webp", "gif", "bmp"]);
        }
        DocTool::OdtToWord => {
            d = d.add_filter("odt", &["odt"]);
        }
        DocTool::WordToOdt => {
            d = d.add_filter("docx", &["docx"]);
        }
        DocTool::AddImage => {
            d = d.add_filter("pdf", &["pdf"]);
        }
        _ if tool.needs_pdf() => {
            d = d.add_filter("pdf", &["pdf"]);
        }
        _ => {
            d = d.add_filter("documents", &["pdf", "png", "jpg", "jpeg", "docx", "odt"]);
        }
    }
    d
}

#[derive(Clone, Debug, Default)]
pub struct ToolParams {
    pub split_ranges: String,
    pub split_every: String,
    pub rotate_degrees: i32,
    pub page_spec: String,
    pub reorder_spec: String,
    pub crop_rect: String,
    pub crop_page: u32,
    pub stamp_text: String,
    pub stamp_page: u32,
    pub sign_name: String,
    pub sign_reason: String,
    pub sign_page: u32,
    pub handwriting_ocr: bool,
    pub stamp_image: Option<PathBuf>,
}

impl ToolParams {
    pub fn with_page_count(page_count: u32) -> Self {
        let all = if page_count > 0 {
            format!("1-{page_count}")
        } else {
            "1".to_owned()
        };
        let order: String = (1..=page_count)
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");
        Self {
            page_spec: all.clone(),
            crop_page: 1,
            stamp_page: 1,
            sign_page: 1,
            rotate_degrees: 90,
            stamp_text: "CONFIDENTIAL".to_owned(),
            sign_name: "Signed".to_owned(),
            sign_reason: "Approved".to_owned(),
            reorder_spec: order,
            crop_rect: format_rect(default_crop_rect()),
            ..Default::default()
        }
    }
}

fn format_rect(r: PdfRect) -> String {
    let w = r.urx - r.llx;
    let h = r.ury - r.lly;
    format!(
        "{},{},{},{}",
        r.llx as i32, r.lly as i32, w as i32, h as i32
    )
}

pub fn tiny_stamp_png() -> Vec<u8> {
    let img = image::RgbaImage::from_fn(48, 48, |x, y| {
        if x < 3 || y < 3 || x >= 45 || y >= 45 {
            image::Rgba([30, 120, 220, 255])
        } else {
            image::Rgba([255, 255, 255, 200])
        }
    });
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    if image::DynamicImage::ImageRgba8(img)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .is_err()
    {
        return Vec::new();
    }
    buf
}

#[allow(clippy::too_many_arguments)]
pub fn run_tool(
    tool: DocTool,
    inputs: &[PathBuf],
    output: &Path,
    params: &ToolParams,
    models_dir: &Path,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<(), String> {
    match tool {
        DocTool::Merge => {
            if inputs.len() < 2 {
                return Err("pick at least two PDFs".to_owned());
            }
            let pdfs: Result<Vec<Pdf>, String> = inputs
                .iter()
                .map(|p| Pdf::load(p).map_err(|e| e.to_string()))
                .collect();
            let mut merged = merge(pdfs?, cancel, progress).map_err(|e| e.to_string())?;
            merged.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::Split => {
            let input = inputs.first().ok_or("no input")?;
            let pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let stem = output
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("part");
            let parent = output.parent().unwrap_or_else(|| Path::new("."));
            let every = params.split_every.trim().parse::<u32>().ok();
            let parts = if let Some(n) = every {
                split_every(&pdf, n, cancel, progress).map_err(|e| e.to_string())?
            } else {
                let groups = if params.split_ranges.trim().is_empty() {
                    vec![PageRanges::parse("1").map_err(|e| e.to_string())?]
                } else {
                    crate::state::parse_tool_split_ranges(&params.split_ranges)?
                };
                split(&pdf, &groups, cancel, progress).map_err(|e| e.to_string())?
            };
            if parts.len() == 1 {
                parts
                    .into_iter()
                    .next()
                    .ok_or("empty split")?
                    .save(output)
                    .map_err(|e| e.to_string())?;
            } else {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                for (i, mut part) in parts.into_iter().enumerate() {
                    let path = parent.join(format!("{stem}-{:02}.pdf", i + 1));
                    part.save(&path).map_err(|e| e.to_string())?;
                }
            }
        }
        DocTool::PdfToWord => {
            let input = inputs.first().ok_or("no input")?;
            let opts = ConvertOptions {
                image: Default::default(),
                pdf_to_word: PdfToWordOptions {
                    mode: PdfToWordMode::ExactLook,
                    ..Default::default()
                },
            };
            convert(input, output, &opts, cancel, progress).map_err(|e| e.to_string())?;
        }
        DocTool::Rotate => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let pages = parse_pages(&params.page_spec, pdf.page_count())?;
            rotate(&mut pdf, &pages, params.rotate_degrees, cancel, progress)
                .map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::Delete => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let pages = parse_pages(&params.page_spec, pdf.page_count())?;
            delete_pages(&mut pdf, &pages, cancel, progress).map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::Reorder => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let order = parse_order(&params.reorder_spec)?;
            reorder(&mut pdf, &order, cancel, progress).map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::Crop => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let rect = crate::parse_util::parse_rect(&params.crop_rect)?;
            let pages = vec![params.crop_page.max(1)];
            crop(&mut pdf, &pages, rect, false, cancel, progress).map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::AddText => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            add_text(
                &mut pdf,
                params.stamp_page.max(1),
                72.0,
                72.0,
                14.0,
                &params.stamp_text,
                docsuite_pdf::Rgb(0.0, 0.0, 0.0),
                cancel,
                progress,
            )
            .map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::AddImage => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let img_bytes = if let Some(p) = &params.stamp_image {
                std::fs::read(p).map_err(|e| e.to_string())?
            } else {
                tiny_stamp_png()
            };
            let rect = default_stamp_rect();
            add_image(
                &mut pdf,
                params.stamp_page.max(1),
                rect,
                &img_bytes,
                cancel,
                progress,
            )
            .map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::Sign => {
            let input = inputs.first().ok_or("no input")?;
            let mut pdf = Pdf::load(input).map_err(|e| e.to_string())?;
            let stamp = SignatureStamp {
                name: params.sign_name.clone(),
                reason: params.sign_reason.clone(),
                date: String::new(),
                image: Some(tiny_stamp_png()),
            };
            visible_signature(
                &mut pdf,
                params.sign_page.max(1),
                default_sign_rect(),
                stamp,
                cancel,
                progress,
            )
            .map_err(|e| e.to_string())?;
            pdf.save(output).map_err(|e| e.to_string())?;
        }
        DocTool::Ocr => {
            let input = inputs.first().ok_or("no input")?;
            let rec =
                open_recognizer(models_dir, params.handwriting_ocr).map_err(|e| e.to_string())?;
            let ocr = Arc::new(docsuite_convert::RecognizerPageOcr {
                recognizer: rec,
                highlight_low_confidence: false,
            });
            let opts = PdfTextOptions {
                pages: None,
                ocr: true,
                page_ocr: Some(ocr),
            };
            pdf_to_searchable_pdf(input, output, &opts, cancel, progress)
                .map_err(|e| e.to_string())?;
        }
        DocTool::ImagesToPdf => {
            if inputs.is_empty() {
                return Err("pick images".to_owned());
            }
            if inputs.len() == 1 {
                image_to_pdf(
                    &inputs[0],
                    output,
                    &ImageToPdfOptions::default(),
                    cancel,
                    progress,
                )
                .map_err(|e| e.to_string())?;
            } else {
                let mut temps = Vec::new();
                let work = output.parent().unwrap_or_else(|| Path::new("."));
                for (i, inp) in inputs.iter().enumerate() {
                    let tmp = work.join(format!(".docapp-img-{i}.pdf"));
                    image_to_pdf(inp, &tmp, &ImageToPdfOptions::default(), cancel, progress)
                        .map_err(|e| e.to_string())?;
                    temps.push(tmp);
                }
                let pdfs: Result<Vec<Pdf>, String> = temps
                    .iter()
                    .map(|p| Pdf::load(p).map_err(|e| e.to_string()))
                    .collect();
                let mut merged = merge(pdfs?, cancel, progress).map_err(|e| e.to_string())?;
                merged.save(output).map_err(|e| e.to_string())?;
                for t in temps {
                    let _ = std::fs::remove_file(t);
                }
            }
        }
        DocTool::OdtToWord => {
            let input = inputs.first().ok_or("no input")?;
            docsuite_convert::odt_to_docx(input, output, cancel, progress)
                .map_err(|e| e.to_string())?;
        }
        DocTool::WordToOdt => {
            let input = inputs.first().ok_or("no input")?;
            docsuite_convert::docx_to_odt(input, output, cancel, progress)
                .map_err(|e| e.to_string())?;
        }
        DocTool::ExtractText => {
            let input = inputs.first().ok_or("no input")?;
            let opts = PdfTextOptions {
                pages: None,
                ocr: false,
                page_ocr: None,
            };
            pdf_to_txt(input, output, &opts, cancel, progress).map_err(|e| e.to_string())?;
        }
        DocTool::PdfToImages => {
            let input = inputs.first().ok_or("no input")?;
            std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
            let opts = PdfToImageOptions {
                dpi: 150.0,
                pages: None,
                format: Format::Png,
            };
            pdf_to_image(input, output, &opts, cancel, progress).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn open_parent_dir(path: &Path) {
    let parent = path.parent().unwrap_or(path);
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(parent).spawn();
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(parent).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(parent).spawn();
    }
}

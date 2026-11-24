#![allow(clippy::too_many_arguments)]

use std::ffi::c_char;
use std::path::Path;
use std::sync::{Arc, Mutex};

use docsuite_convert::extract::extract_page;
use docsuite_convert::{
    ImageToPdfOptions, PdfToWordMode, PdfToWordOptions, docx_to_odt as convert_docx_to_odt,
    image_to_pdf, odt_to_docx as convert_odt_to_docx, pdf_to_docx,
};
use docsuite_core::{CancelToken, Error, Progress, ProgressSink};
use docsuite_models::{
    DETECTION_FILE, DETECTION_MODEL_URL, DETECTION_SHA256, RECOGNITION_FILE, RECOGNITION_MODEL_URL,
    RECOGNITION_SHA256,
};
use docsuite_ocr::{FieldType, predict_type, status as models_status_fn};
use docsuite_pdf::{
    PageRanges, Pdf, PdfRect, Rgb, SignatureStamp, add_image, add_text, crop, delete_pages, merge,
    pdf_info, reorder, rotate, split, visible_signature,
};

#[cfg(feature = "ocrs")]
use docsuite_convert::{PdfTextOptions, RecognizerPageOcr, image_to_txt, pdf_to_searchable_pdf};
#[cfg(feature = "ocrs")]
use docsuite_ocr::OcrsRecognizer;

#[unsafe(no_mangle)]
pub extern "C" fn docsuite_version() -> *const c_char {
    docsuite_core::version_cstr().as_ptr().cast()
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum DocsuiteError {
    #[error("io: {detail}")]
    Io { detail: String },
    #[error("pdf: {detail}")]
    Pdf { detail: String },
    #[error("parse: {detail}")]
    Parse { detail: String },
    #[error("unsupported: {detail}")]
    Unsupported { detail: String },
    #[error("invalid input: {detail}")]
    InvalidInput { detail: String },
    #[error("cancelled")]
    Cancelled,
    #[error("backend: {detail}")]
    Backend { detail: String },
    #[error("ocr: {detail}")]
    Ocr { detail: String },
    #[error("convert: {detail}")]
    Convert { detail: String },
}

impl From<Error> for DocsuiteError {
    fn from(value: Error) -> Self {
        match value {
            Error::Io(e) => DocsuiteError::Io {
                detail: e.to_string(),
            },
            Error::Pdf(s) => DocsuiteError::Pdf { detail: s },
            Error::Parse(s) => DocsuiteError::Parse { detail: s },
            Error::Unsupported(s) => DocsuiteError::Unsupported { detail: s },
            Error::InvalidInput(s) => DocsuiteError::InvalidInput { detail: s },
            Error::Cancelled => DocsuiteError::Cancelled,
            Error::Backend(s) => DocsuiteError::Backend { detail: s },
            Error::Ocr(s) => DocsuiteError::Ocr { detail: s },
            Error::Convert(s) => DocsuiteError::Convert { detail: s },
        }
    }
}

#[derive(uniffi::Record)]
pub struct DetectTypeResult {
    pub field_type: String,
    pub normalized: Option<String>,
}

#[derive(uniffi::Record)]
pub struct PdfRectFfi {
    pub llx: f64,
    pub lly: f64,
    pub urx: f64,
    pub ury: f64,
}

#[derive(uniffi::Record)]
pub struct ModelsStatus {
    pub detection_present: bool,
    pub recognition_present: bool,
}

impl From<docsuite_ocr::ModelStatus> for ModelsStatus {
    fn from(s: docsuite_ocr::ModelStatus) -> Self {
        Self {
            detection_present: s.detection,
            recognition_present: s.recognition,
        }
    }
}

impl From<PdfRectFfi> for PdfRect {
    fn from(r: PdfRectFfi) -> Self {
        Self {
            llx: r.llx,
            lly: r.lly,
            urx: r.urx,
            ury: r.ury,
        }
    }
}

#[uniffi::export(callback_interface)]
pub trait ProgressListener: Send + Sync {
    fn on_progress(&self, done: u64, total: u64, stage: String);
}

#[derive(uniffi::Object)]
pub struct CancelHandle {
    inner: Mutex<CancelToken>,
}

#[uniffi::export]
impl CancelHandle {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(CancelToken::new()),
        })
    }

    pub fn cancel(&self) {
        if let Ok(guard) = self.inner.lock() {
            guard.cancel();
        }
    }
}

fn token_from(handle: &CancelHandle) -> CancelToken {
    match handle.inner.lock() {
        Ok(g) => g.clone(),
        Err(p) => p.into_inner().clone(),
    }
}

fn progress_sink(listener: Option<Box<dyn ProgressListener>>) -> Option<ProgressSink> {
    listener.map(|l| {
        let listener: Arc<dyn ProgressListener> = Arc::from(l);
        Arc::new(move |p: Progress| {
            listener.on_progress(p.done, p.total, p.stage);
        }) as ProgressSink
    })
}

fn field_label(ft: FieldType) -> String {
    match ft {
        FieldType::Date => "date",
        FieldType::Currency => "currency",
        FieldType::CodeId => "code_id",
        FieldType::Email => "email",
        FieldType::Number => "number",
        FieldType::Text => "text",
    }
    .to_owned()
}

fn page_plain(page: &docsuite_convert::ExtractedPage) -> String {
    let mut out = String::new();
    for line in &page.text {
        for (i, run) in line.runs.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&run.text);
        }
        if !line.runs.is_empty() {
            out.push('\n');
        }
    }
    out
}

#[uniffi::export]
pub fn version() -> String {
    docsuite_core::VERSION.to_owned()
}

#[uniffi::export]
pub fn page_count(path: String) -> Result<u32, DocsuiteError> {
    let pdf = Pdf::load(path)?;
    Ok(pdf_info(&pdf)?.pages)
}

#[uniffi::export(name = "split")]
pub fn split_document(
    path: String,
    ranges: String,
    outdir: String,
    single: bool,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let pdf = Pdf::load(&path)?;
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let parsed_groups = if single {
        let parsed = PageRanges::parse(&ranges)?;
        vec![parsed]
    } else {
        PageRanges::parse_groups(&ranges)?
    };
    let parts = split(&pdf, &parsed_groups, &token, &sink)?;
    std::fs::create_dir_all(&outdir).map_err(|e| DocsuiteError::Io {
        detail: e.to_string(),
    })?;
    let stem = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("doc");
    for (i, mut part) in parts.into_iter().enumerate() {
        let out = Path::new(&outdir).join(format!("{stem}-part{:03}.pdf", i + 1));
        part.save(&out)?;
    }
    Ok(())
}

#[uniffi::export(name = "merge")]
pub fn merge_documents(
    paths: Vec<String>,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    if paths.is_empty() {
        return Err(DocsuiteError::InvalidInput {
            detail: "no inputs".to_owned(),
        });
    }
    let pdfs: Vec<Pdf> = paths.iter().map(Pdf::load).collect::<Result<_, _>>()?;
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut merged = merge(pdfs, &token, &sink)?;
    merged.save(output)?;
    Ok(())
}

#[uniffi::export(name = "rotate")]
pub fn rotate_document(
    path: String,
    pages: Vec<u32>,
    degrees: i32,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    rotate(&mut pdf, &pages, degrees, &token, &sink)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn delete_pages_ffi(
    path: String,
    pages: Vec<u32>,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    delete_pages(&mut pdf, &pages, &token, &sink)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn reorder_pages(
    path: String,
    order: Vec<u32>,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    reorder(&mut pdf, &order, &token, &sink)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn crop_pages(
    path: String,
    pages: Vec<u32>,
    rect: PdfRectFfi,
    expand: bool,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    crop(&mut pdf, &pages, rect.into(), expand, &token, &sink)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn stamp_text(
    path: String,
    page: u32,
    x: f64,
    y: f64,
    size: f64,
    text: String,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    add_text(
        &mut pdf,
        page,
        x,
        y,
        size,
        &text,
        Rgb(0.0, 0.0, 0.0),
        &token,
        &sink,
    )?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn stamp_image(
    path: String,
    page: u32,
    rect: PdfRectFfi,
    image_path: String,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let image_bytes = std::fs::read(&image_path).map_err(|e| DocsuiteError::Io {
        detail: e.to_string(),
    })?;
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    add_image(&mut pdf, page, rect.into(), &image_bytes, &token, &sink)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn visible_signature_ffi(
    path: String,
    page: u32,
    rect: PdfRectFfi,
    name: String,
    reason: String,
    image_path: Option<String>,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let image = match image_path {
        Some(p) => Some(std::fs::read(&p).map_err(|e| DocsuiteError::Io {
            detail: e.to_string(),
        })?),
        None => None,
    };
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mut pdf = Pdf::load(&path)?;
    let stamp = SignatureStamp {
        name,
        reason,
        date: String::new(),
        image,
    };
    visible_signature(&mut pdf, page, rect.into(), stamp, &token, &sink)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export]
pub fn image_to_pdf_ffi(
    input: String,
    output: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    image_to_pdf(
        Path::new(&input),
        Path::new(&output),
        &ImageToPdfOptions::default(),
        &token,
        &sink,
    )?;
    Ok(())
}

#[uniffi::export]
pub fn odt_to_docx(
    path: String,
    out: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    convert_odt_to_docx(Path::new(&path), Path::new(&out), &token, &sink)?;
    Ok(())
}

#[uniffi::export]
pub fn docx_to_odt(
    path: String,
    out: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    convert_docx_to_odt(Path::new(&path), Path::new(&out), &token, &sink)?;
    Ok(())
}

#[uniffi::export(name = "pdf_to_docx")]
pub fn ffi_pdf_to_docx(
    path: String,
    out: String,
    editable: bool,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let mode = if editable {
        PdfToWordMode::Editable
    } else {
        PdfToWordMode::ExactLook
    };
    let opts = PdfToWordOptions {
        mode,
        ..PdfToWordOptions::default()
    };
    pdf_to_docx(Path::new(&path), Path::new(&out), &opts, &token, &sink)?;
    Ok(())
}

#[cfg(feature = "ocrs")]
#[uniffi::export]
pub fn ocr_image(
    path: String,
    models_dir: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<String, DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let rec = OcrsRecognizer::with_dir(Path::new(&models_dir))?;
    let ocr = RecognizerPageOcr {
        recognizer: Arc::new(rec),
        highlight_low_confidence: false,
    };
    let tmp = std::env::temp_dir().join(format!("docsuite-ocr-{}.txt", std::process::id()));
    image_to_txt(Path::new(&path), &tmp, &ocr, &token, &sink)?;
    let text = std::fs::read_to_string(&tmp).map_err(|e| DocsuiteError::Io {
        detail: e.to_string(),
    })?;
    let _ = std::fs::remove_file(&tmp);
    Ok(text)
}

#[cfg(feature = "ocrs")]
#[uniffi::export]
pub fn make_searchable(
    path: String,
    out: String,
    models_dir: String,
    cancel: Arc<CancelHandle>,
    progress: Option<Box<dyn ProgressListener>>,
) -> Result<(), DocsuiteError> {
    let token = token_from(&cancel);
    let sink = progress_sink(progress);
    let rec = OcrsRecognizer::with_dir(Path::new(&models_dir))?;
    let ocr = Arc::new(RecognizerPageOcr {
        recognizer: Arc::new(rec),
        highlight_low_confidence: false,
    });
    let opts = PdfTextOptions {
        pages: None,
        ocr: true,
        page_ocr: Some(ocr),
    };
    pdf_to_searchable_pdf(Path::new(&path), Path::new(&out), &opts, &token, &sink)?;
    Ok(())
}

#[uniffi::export]
pub fn extract_text_layer(path: String, page: u32) -> Result<String, DocsuiteError> {
    let pdf = Pdf::load(&path)?;
    let doc = pdf.lopdf_document();
    let page_id = pdf.page_id(page)?;
    let extracted = extract_page(doc, page_id)?;
    Ok(page_plain(&extracted))
}

#[uniffi::export]
pub fn detect_type(text: String) -> DetectTypeResult {
    let guess = predict_type(&text);
    DetectTypeResult {
        field_type: field_label(guess.field_type),
        normalized: guess.normalized,
    }
}

#[uniffi::export]
pub fn models_status(dir: String) -> ModelsStatus {
    models_status_fn(Path::new(&dir)).into()
}

#[uniffi::export]
pub fn detection_model_url() -> String {
    DETECTION_MODEL_URL.to_owned()
}

#[uniffi::export]
pub fn recognition_model_url() -> String {
    RECOGNITION_MODEL_URL.to_owned()
}

#[uniffi::export]
pub fn detection_model_sha256() -> String {
    DETECTION_SHA256.to_owned()
}

#[uniffi::export]
pub fn recognition_model_sha256() -> String {
    RECOGNITION_SHA256.to_owned()
}

#[uniffi::export]
pub fn detection_model_filename() -> String {
    DETECTION_FILE.to_owned()
}

#[uniffi::export]
pub fn recognition_model_filename() -> String {
    RECOGNITION_FILE.to_owned()
}

uniffi::setup_scaffolding!();

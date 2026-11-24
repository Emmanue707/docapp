use std::ffi::c_char;
use std::path::Path;

use docsuite_convert::{PdfToWordMode, PdfToWordOptions, pdf_to_docx};
use docsuite_core::Error;
use docsuite_ocr::{FieldType, predict_type};
use docsuite_pdf::{PageRanges, Pdf, merge, pdf_info, rotate, split};

#[unsafe(no_mangle)]
pub extern "C" fn docsuite_version() -> *const c_char {
    docsuite_core::version_cstr().as_ptr().cast()
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum DocsuiteError {
    #[error("io: {message}")]
    Io { message: String },
    #[error("pdf: {message}")]
    Pdf { message: String },
    #[error("parse: {message}")]
    Parse { message: String },
    #[error("unsupported: {message}")]
    Unsupported { message: String },
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("cancelled")]
    Cancelled,
    #[error("backend: {message}")]
    Backend { message: String },
    #[error("ocr: {message}")]
    Ocr { message: String },
    #[error("convert: {message}")]
    Convert { message: String },
}

impl From<Error> for DocsuiteError {
    fn from(value: Error) -> Self {
        match value {
            Error::Io(e) => DocsuiteError::Io {
                message: e.to_string(),
            },
            Error::Pdf(s) => DocsuiteError::Pdf { message: s },
            Error::Parse(s) => DocsuiteError::Parse { message: s },
            Error::Unsupported(s) => DocsuiteError::Unsupported { message: s },
            Error::InvalidInput(s) => DocsuiteError::InvalidInput { message: s },
            Error::Cancelled => DocsuiteError::Cancelled,
            Error::Backend(s) => DocsuiteError::Backend { message: s },
            Error::Ocr(s) => DocsuiteError::Ocr { message: s },
            Error::Convert(s) => DocsuiteError::Convert { message: s },
        }
    }
}

#[derive(uniffi::Record)]
pub struct DetectTypeResult {
    pub field_type: String,
    pub normalized: Option<String>,
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
pub fn split_document(path: String, ranges: String, outdir: String) -> Result<(), DocsuiteError> {
    let pdf = Pdf::load(&path)?;
    let parsed = PageRanges::parse(&ranges)?;
    let cancel = docsuite_core::CancelToken::new();
    let parts = split(&pdf, &[parsed], &cancel, &None)?;
    std::fs::create_dir_all(&outdir).map_err(|e| DocsuiteError::Io {
        message: e.to_string(),
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
pub fn merge_documents(paths: Vec<String>, output: String) -> Result<(), DocsuiteError> {
    if paths.is_empty() {
        return Err(DocsuiteError::InvalidInput {
            message: "no inputs".to_owned(),
        });
    }
    let pdfs: Vec<Pdf> = paths.iter().map(Pdf::load).collect::<Result<_, _>>()?;
    let cancel = docsuite_core::CancelToken::new();
    let mut merged = merge(pdfs, &cancel, &None)?;
    merged.save(output)?;
    Ok(())
}

#[uniffi::export(name = "rotate")]
pub fn rotate_document(
    path: String,
    pages: Vec<u32>,
    degrees: i32,
    output: String,
) -> Result<(), DocsuiteError> {
    let cancel = docsuite_core::CancelToken::new();
    let mut pdf = Pdf::load(&path)?;
    rotate(&mut pdf, &pages, degrees, &cancel, &None)?;
    pdf.save(output)?;
    Ok(())
}

#[uniffi::export(name = "pdf_to_docx")]
pub fn ffi_pdf_to_docx(path: String, out: String, editable: bool) -> Result<(), DocsuiteError> {
    let cancel = docsuite_core::CancelToken::new();
    let mode = if editable {
        PdfToWordMode::Editable
    } else {
        PdfToWordMode::ExactLook
    };
    let opts = PdfToWordOptions {
        mode,
        ..PdfToWordOptions::default()
    };
    pdf_to_docx(Path::new(&path), Path::new(&out), &opts, &cancel, &None)?;
    Ok(())
}

#[uniffi::export]
pub fn detect_type(text: String) -> DetectTypeResult {
    let guess = predict_type(&text);
    DetectTypeResult {
        field_type: field_label(guess.field_type),
        normalized: guess.normalized,
    }
}

uniffi::setup_scaffolding!();

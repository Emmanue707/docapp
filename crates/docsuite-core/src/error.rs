use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Pdf(String),
    #[error("{0}")]
    Parse(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("cancelled")]
    Cancelled,
    #[error("{0}")]
    Backend(String),
    #[error("{0}")]
    Ocr(String),
    #[error("{0}")]
    Convert(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn copy_error(err: &Error) -> Error {
    match err {
        Error::Io(e) => Error::Io(std::io::Error::new(e.kind(), e.to_string())),
        Error::Pdf(s) => Error::Pdf(s.clone()),
        Error::Parse(s) => Error::Parse(s.clone()),
        Error::Unsupported(s) => Error::Unsupported(s.clone()),
        Error::InvalidInput(s) => Error::InvalidInput(s.clone()),
        Error::Cancelled => Error::Cancelled,
        Error::Backend(s) => Error::Backend(s.clone()),
        Error::Ocr(s) => Error::Ocr(s.clone()),
        Error::Convert(s) => Error::Convert(s.clone()),
    }
}

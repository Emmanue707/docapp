use docsuite_core::Error;

pub fn map_err(err: lopdf::Error) -> Error {
    Error::Pdf(err.to_string())
}

pub fn io_err(err: std::io::Error) -> Error {
    Error::Io(err)
}

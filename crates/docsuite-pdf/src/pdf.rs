use std::path::Path;

use docsuite_core::{Error, Result};
use lopdf::Document;

use crate::lopdf_map::{io_err, map_err};

pub struct Pdf {
    pub(crate) doc: Document,
}

impl Pdf {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        Document::load(path)
            .map_err(map_err)
            .map(|doc| Self { doc })
    }

    pub fn load_bytes(bytes: &[u8]) -> Result<Self> {
        Document::load_mem(bytes)
            .map_err(map_err)
            .map(|doc| Self { doc })
    }

    pub fn from_document(doc: Document) -> Self {
        Self { doc }
    }

    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.compress_streams();
        self.doc.save(path).map_err(io_err)?;
        Ok(())
    }

    pub fn to_bytes(&mut self) -> Result<Vec<u8>> {
        self.compress_streams();
        let mut buf = Vec::new();
        self.doc.save_to(&mut buf).map_err(io_err)?;
        Ok(buf)
    }

    pub fn page_count(&self) -> u32 {
        self.doc.get_pages().len() as u32
    }

    pub(crate) fn compress_streams(&mut self) {
        self.doc.compress();
    }

    pub(crate) fn into_document(self) -> Document {
        self.doc
    }

    pub(crate) fn document_mut(&mut self) -> &mut Document {
        &mut self.doc
    }

    pub(crate) fn document(&self) -> &Document {
        &self.doc
    }

    pub fn page_id(&self, one_based: u32) -> Result<lopdf::ObjectId> {
        self.doc
            .get_pages()
            .get(&one_based)
            .copied()
            .ok_or_else(|| Error::InvalidInput(format!("page {one_based} out of range")))
    }
}

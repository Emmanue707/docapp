use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use docsuite_core::{Error, Result};
use zip::ZipArchive;
use zip::{ZipWriter, write::SimpleFileOptions};

pub fn read_zip_bytes(zip_bytes: &[u8], path: &str) -> Result<Vec<u8>> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(cursor).map_err(|e| Error::Parse(e.to_string()))?;
    let mut file = archive
        .by_name(path)
        .map_err(|_| Error::Parse(format!("missing {path}")))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).map_err(Error::from)?;
    Ok(buf)
}

pub fn sniff_ooxml_kind(pkg: &[u8]) -> Result<&'static str> {
    if let Ok(mime) = read_zip_bytes(pkg, "mimetype") {
        let s = String::from_utf8_lossy(&mime);
        if s.contains("opendocument.text") {
            return Ok("odt");
        }
    }
    if read_zip_bytes(pkg, "word/document.xml").is_ok() {
        return Ok("docx");
    }
    if read_zip_bytes(pkg, "content.xml").is_ok() {
        return Ok("odt");
    }
    Err(Error::Parse("unknown office package".to_owned()))
}

pub struct OdfWriter {
    zip: ZipWriter<Cursor<Vec<u8>>>,
    stored: Vec<(String, Vec<u8>)>,
    deflated: Vec<(String, Vec<u8>)>,
}

impl OdfWriter {
    pub fn new() -> Self {
        Self {
            zip: ZipWriter::new(Cursor::new(Vec::new())),
            stored: Vec::new(),
            deflated: Vec::new(),
        }
    }

    pub fn write_stored(&mut self, path: &str, data: &[u8]) {
        self.stored.push((path.to_owned(), data.to_vec()));
    }

    pub fn write_deflated(&mut self, path: &str, data: &[u8]) {
        self.deflated.push((path.to_owned(), data.to_vec()));
    }

    pub fn finish(mut self, out_path: &std::path::Path) -> Result<()> {
        let stored_opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        let deflated_opts =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (path, data) in &self.stored {
            self.zip.start_file(path, stored_opts).map_err(zip_err)?;
            self.zip.write_all(data).map_err(Error::from)?;
        }
        for (path, data) in &self.deflated {
            self.zip.start_file(path, deflated_opts).map_err(zip_err)?;
            self.zip.write_all(data).map_err(Error::from)?;
        }
        let cursor = self.zip.finish().map_err(zip_err)?;
        std::fs::write(out_path, cursor.into_inner()).map_err(Error::from)?;
        Ok(())
    }
}

pub struct OoxWriter {
    zip: ZipWriter<std::fs::File>,
    opts: SimpleFileOptions,
}

impl OoxWriter {
    pub fn create(path: &std::path::Path) -> Result<Self> {
        let file = std::fs::File::create(path).map_err(Error::from)?;
        Ok(Self {
            zip: ZipWriter::new(file),
            opts: SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
        })
    }

    pub fn write(&mut self, path: &str, data: &[u8]) -> Result<()> {
        self.zip.start_file(path, self.opts).map_err(zip_err)?;
        self.zip.write_all(data).map_err(Error::from)?;
        Ok(())
    }

    pub fn write_str(&mut self, path: &str, data: &str) -> Result<()> {
        self.write(path, data.as_bytes())
    }

    pub fn finish(self) -> Result<()> {
        self.zip.finish().map_err(zip_err)?;
        Ok(())
    }
}

fn zip_err(e: zip::result::ZipError) -> Error {
    Error::Convert(e.to_string())
}

pub fn collect_rels(pkg: &[u8], rels_path: &str) -> Result<HashMap<String, String>> {
    let xml = read_zip_bytes(pkg, rels_path)?;
    let text = String::from_utf8_lossy(&xml);
    let mut map = HashMap::new();
    for chunk in text.split("<Relationship ") {
        let id = attr_val(chunk, "Id");
        let target = attr_val(chunk, "Target");
        if let (Some(id), Some(target)) = (id, target) {
            map.insert(id, target);
        }
    }
    Ok(map)
}

pub fn attr_val(s: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = s.find(&needle)? + needle.len();
    let end = s[start..].find('"')? + start;
    Some(s[start..end].to_owned())
}

pub fn local_name(qname: &[u8]) -> &[u8] {
    match qname.iter().position(|&b| b == b':') {
        Some(i) => &qname[i + 1..],
        None => qname,
    }
}

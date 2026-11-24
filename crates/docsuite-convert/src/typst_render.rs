use docsuite_core::{Error, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use typst::diag::FileResult;
use typst::foundations::{Bytes, Datetime, Duration};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World};
use typst_kit::fonts::{FontStore, scan, system};
use typst_pdf::{PdfOptions, pdf};

use crate::doc::Document;

const TEMPLATE: &str = include_str!("assets/render.typ");

struct MemWorld {
    library: LazyHash<Library>,
    fonts: FontStore,
    main: FileId,
    sources: HashMap<FileId, Source>,
    files: HashMap<FileId, Bytes>,
}

impl MemWorld {
    fn new(main_path: &str, main_src: String, extras: Vec<(String, Bytes)>) -> Result<Self> {
        let library = LazyHash::new(Library::builder().build());
        let mut fonts = FontStore::new();
        for dir in font_dirs() {
            if dir.is_dir() {
                fonts.extend(scan(&dir));
            }
        }
        fonts.extend(system());
        let mut sources = HashMap::new();
        let mut files = HashMap::new();
        let main_id = file_id(main_path)?;
        sources.insert(main_id, Source::new(main_id, main_src));
        for (path, data) in extras {
            let id = file_id(&path)?;
            if path.ends_with(".typ") {
                let text =
                    String::from_utf8(data.to_vec()).map_err(|e| Error::Convert(e.to_string()))?;
                sources.insert(id, Source::new(id, text));
            } else {
                files.insert(id, data);
            }
        }
        Ok(Self {
            library,
            fonts,
            main: main_id,
            sources,
            files,
        })
    }
}

#[allow(deprecated)]
impl World for MemWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        self.fonts.book()
    }

    fn main(&self) -> FileId {
        self.main
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        self.sources
            .get(&id)
            .cloned()
            .ok_or_else(|| typst::diag::FileError::NotFound(id.vpath().as_rooted_path().into()))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(b) = self.files.get(&id) {
            return Ok(b.clone());
        }
        Err(typst::diag::FileError::NotFound(
            id.vpath().as_rooted_path().into(),
        ))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.font(index)
    }

    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        None
    }
}

fn file_id(path: &str) -> Result<FileId> {
    let vp = VirtualPath::new(path).map_err(|e| Error::Convert(e.to_string()))?;
    let rooted = RootedPath::new(VirtualRoot::Project, vp);
    Ok(FileId::new(rooted))
}

fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    dirs.push(PathBuf::from("/usr/share/fonts"));
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/fonts"));
    }
    #[cfg(target_os = "windows")]
    dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/System/Library/Fonts"));
        dirs.push(PathBuf::from("/Library/Fonts"));
    }
    dirs
}

pub fn document_to_pdf(doc: &Document, output: &Path) -> Result<()> {
    let json = crate::doc::typst_json::to_json(doc).map_err(|e| Error::Convert(e.to_string()))?;
    let mut files: Vec<(String, Bytes)> = vec![
        ("doc.json".to_owned(), Bytes::new(json.into_bytes())),
        (
            "render.typ".to_owned(),
            Bytes::new(TEMPLATE.as_bytes().to_vec()),
        ),
    ];
    for (path, data) in crate::doc::typst_json::collect_image_files(doc) {
        files.push((path, Bytes::new(data)));
    }
    let world = MemWorld::new("render.typ", TEMPLATE.to_owned(), files)?;
    let warned = typst::compile(&world);
    let paged = warned.output.map_err(|e| Error::Convert(format_diags(e)))?;
    let pdf_opts = PdfOptions {
        tagged: false,
        ..PdfOptions::default()
    };
    let bytes = pdf(&paged, &pdf_opts).map_err(|e| Error::Convert(format_diags(e)))?;
    std::fs::write(output, bytes).map_err(Error::from)?;
    Ok(())
}

fn format_diags(err: impl std::fmt::Debug) -> String {
    format!("{err:?}")
}

pub fn read_document_to_pdf(path: &Path, output: &Path) -> Result<()> {
    let pkg = std::fs::read(path).map_err(docsuite_core::Error::from)?;
    let kind = crate::ooxf::sniff_ooxml_kind(&pkg)?;
    let doc = match kind {
        "docx" => crate::docx_read::read_docx_package(&pkg)?,
        "odt" => crate::odt_read::read_odt_package(&pkg)?,
        _ => return Err(Error::Unsupported("not office document".to_owned())),
    };
    document_to_pdf(&doc, output)
}

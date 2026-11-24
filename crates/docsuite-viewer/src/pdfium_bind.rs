#[cfg(feature = "pdfium")]
pub(crate) mod imp {
    use std::env;
    use std::path::{Path, PathBuf};

    use docsuite_core::{Error, Result};
    use pdfium_render::prelude::*;

    pub fn bind_pdfium() -> Result<Pdfium> {
        let name = Pdfium::pdfium_platform_library_name();
        let name_str = name.to_string_lossy();
        if let Ok(exe) = env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let path = dir.join(&name);
            if path.is_file() {
                return bind_at(&path);
            }
        }
        if let Ok(dir) = env::var("DOCAPP_PDFIUM") {
            let path = PathBuf::from(dir).join(&name);
            if path.is_file() {
                return bind_at(&path);
            }
            return Err(Error::Backend(format!(
                "DOCAPP_PDFIUM set but {} not found",
                path.display()
            )));
        }
        Pdfium::bind_to_system_library()
            .map(Pdfium::new)
            .map_err(|e| {
                Error::Backend(format!(
                    "pdfium library missing ({name_str}); set DOCAPP_PDFIUM or place next to executable: {e}"
                ))
            })
    }

    fn bind_at(path: &Path) -> Result<Pdfium> {
        Pdfium::bind_to_library(path)
            .map(Pdfium::new)
            .map_err(|e| Error::Backend(format!("load {}: {e}", path.display())))
    }
}

#[cfg(feature = "pdfium")]
pub(crate) use imp::bind_pdfium;

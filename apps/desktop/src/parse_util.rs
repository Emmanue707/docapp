use docsuite_pdf::{PageRanges, PdfRect};

pub fn parse_pages(spec: &str, page_count: u32) -> Result<Vec<u32>, String> {
    PageRanges::parse(spec)
        .map_err(|e| e.to_string())?
        .pages(page_count)
        .map_err(|e| e.to_string())
}

pub fn parse_order(spec: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let page: u32 = part.parse().map_err(|_| format!("invalid page '{part}'"))?;
        if page == 0 {
            return Err("page numbers are 1-based".to_owned());
        }
        out.push(page);
    }
    if out.is_empty() {
        return Err("empty page order".to_owned());
    }
    Ok(out)
}

pub fn parse_rect(spec: &str) -> Result<PdfRect, String> {
    let mut parts = spec.split(',');
    let x: f64 = parts
        .next()
        .ok_or("rect x")?
        .trim()
        .parse()
        .map_err(|_| "rect x".to_owned())?;
    let y: f64 = parts
        .next()
        .ok_or("rect y")?
        .trim()
        .parse()
        .map_err(|_| "rect y".to_owned())?;
    let w: f64 = parts
        .next()
        .ok_or("rect width")?
        .trim()
        .parse()
        .map_err(|_| "rect width".to_owned())?;
    let h: f64 = parts
        .next()
        .ok_or("rect height")?
        .trim()
        .parse()
        .map_err(|_| "rect height".to_owned())?;
    Ok(PdfRect {
        llx: x,
        lly: y,
        urx: x + w,
        ury: y + h,
    })
}

pub fn default_crop_rect() -> PdfRect {
    PdfRect {
        llx: 36.0,
        lly: 36.0,
        urx: 559.0,
        ury: 806.0,
    }
}

pub fn default_stamp_rect() -> PdfRect {
    PdfRect {
        llx: 72.0,
        lly: 72.0,
        urx: 220.0,
        ury: 160.0,
    }
}

pub fn default_sign_rect() -> PdfRect {
    PdfRect {
        llx: 72.0,
        lly: 600.0,
        urx: 260.0,
        ury: 720.0,
    }
}

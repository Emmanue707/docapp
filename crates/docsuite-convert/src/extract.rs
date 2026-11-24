use std::collections::HashMap;

use docsuite_core::{Error, Result};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, ObjectId, Stream};

use crate::fonts::PdfFont;
use crate::matrix::Matrix;
use crate::types::{
    ExtractedPage, ImageKind, PathPoint, PathSegment, PlacedImage, RgbColor, TextLine, TextRun,
    VectorPath,
};

#[derive(Clone)]
enum PaintSpace {
    Gray,
    Rgb,
    Cmyk,
    Indexed(Vec<(f64, f64, f64)>),
}

#[derive(Clone)]
struct GfxState {
    ctm: Matrix,
    fill_rgb: (f64, f64, f64),
    stroke_rgb: (f64, f64, f64),
    fill_space: PaintSpace,
    stroke_space: PaintSpace,
    line_width: f64,
    tm: Matrix,
    text_line_y: f64,
    font_key: String,
    font_size: f64,
    char_space: f64,
    word_space: f64,
    h_scale: f64,
    render_mode: i32,
    fonts: HashMap<String, PdfFont>,
}

impl GfxState {
    fn new(fonts: HashMap<String, PdfFont>) -> Self {
        Self {
            ctm: Matrix::IDENTITY,
            fill_rgb: (0.0, 0.0, 0.0),
            stroke_rgb: (0.0, 0.0, 0.0),
            fill_space: PaintSpace::Rgb,
            stroke_space: PaintSpace::Rgb,
            line_width: 1.0,
            tm: Matrix::IDENTITY,
            text_line_y: 0.0,
            font_key: String::new(),
            font_size: 12.0,
            char_space: 0.0,
            word_space: 0.0,
            h_scale: 100.0,
            render_mode: 0,
            fonts,
        }
    }
}

struct ExtractCtx {
    page_w: f64,
    page_h: f64,
    text: Vec<TextLine>,
    images: Vec<PlacedImage>,
    vectors: Vec<VectorPath>,
    raw_runs: Vec<TextRun>,
    current_path: Vec<PathSegment>,
    path_start: Option<PathPoint>,
    cur_point: Option<PathPoint>,
}

impl ExtractCtx {
    fn pdf_y_to_top(&self, x: f64, y: f64) -> (f64, f64) {
        (x, self.page_h - y)
    }

    fn push_text_run(&mut self, run: TextRun) {
        if run.hidden {
            self.raw_runs.push(run);
            return;
        }
        self.raw_runs.push(run);
    }
}

pub fn extract_page(doc: &Document, page_id: ObjectId) -> Result<ExtractedPage> {
    let page = docsuite_pdf::page_attr::materialize_page_attributes(doc, page_id)
        .map_err(|e| Error::Pdf(e.to_string()))?;
    let mediabox = box_array(&page, b"MediaBox")?;
    let cropbox = box_array(&page, b"CropBox").unwrap_or(mediabox);
    let rotate = dict_int(&page, b"Rotate") as i32;
    let (width, height) = page_size(cropbox, rotate);
    let fonts = load_page_fonts(doc, &page)?;
    let mut ctx = ExtractCtx {
        page_w: width,
        page_h: height,
        text: Vec::new(),
        images: Vec::new(),
        vectors: Vec::new(),
        raw_runs: Vec::new(),
        current_path: Vec::new(),
        path_start: None,
        cur_point: None,
    };
    let resources = page.get(b"Resources").ok();
    let mut gs = GfxState::new(fonts);
    let content_bytes = doc.get_page_content(page_id);
    let content = Content::decode(&content_bytes).map_err(|e| Error::Pdf(e.to_string()))?;
    interpret_ops(
        doc,
        &mut ctx,
        &mut gs,
        &content.operations,
        resources,
        Matrix::IDENTITY,
    )?;
    ctx.text = merge_into_lines(&ctx.raw_runs, &gs.fonts);
    Ok(ExtractedPage {
        width,
        height,
        rotate,
        text: ctx.text,
        images: ctx.images,
        vectors: ctx.vectors,
    })
}

fn page_size(box4: [f64; 4], rotate: i32) -> (f64, f64) {
    let w = (box4[2] - box4[0]).abs();
    let h = (box4[3] - box4[1]).abs();
    if rotate == 90 || rotate == 270 {
        (h, w)
    } else {
        (w, h)
    }
}

fn box_array(page: &lopdf::Dictionary, key: &[u8]) -> Result<[f64; 4]> {
    let arr = page
        .get(key)
        .map_err(|_| Error::Pdf("missing box".to_owned()))?
        .as_array()
        .map_err(|_| Error::Pdf("bad box".to_owned()))?;
    if arr.len() != 4 {
        return Err(Error::Pdf("bad box len".to_owned()));
    }
    let mut out = [0.0; 4];
    for (i, item) in arr.iter().enumerate() {
        out[i] = object_num(item)?;
    }
    Ok(out)
}

fn object_num(obj: &Object) -> Result<f64> {
    match obj {
        Object::Integer(v) => Ok(*v as f64),
        Object::Real(v) => Ok(f64::from(*v)),
        _ => Err(Error::Pdf("expected number".to_owned())),
    }
}

fn fonts_from_resources(doc: &Document, resources: &Object) -> Result<HashMap<String, PdfFont>> {
    let res_dict = match doc.dereference(resources) {
        Ok((_, Object::Dictionary(d))) => d.clone(),
        Ok((_, Object::Reference(id))) => doc
            .get_dictionary(*id)
            .map_err(|e| Error::Pdf(e.to_string()))?
            .clone(),
        _ => return Ok(HashMap::new()),
    };
    let font_dict = match res_dict.get(b"Font") {
        Ok(f) => f,
        Err(_) => return Ok(HashMap::new()),
    };
    let fonts = match doc.dereference(font_dict) {
        Ok((_, Object::Dictionary(d))) => d.clone(),
        _ => font_dict.as_dict().ok().cloned().unwrap_or_default(),
    };
    let mut out = HashMap::new();
    for (key, value) in fonts.iter() {
        let name = String::from_utf8_lossy(key).into_owned();
        let Object::Reference(id) = value else {
            continue;
        };
        if let Ok(font) = PdfFont::load(doc, *id, &name) {
            out.insert(name, font);
        }
    }
    Ok(out)
}

fn load_page_fonts(doc: &Document, page: &lopdf::Dictionary) -> Result<HashMap<String, PdfFont>> {
    let mut out = HashMap::new();
    let resources = match page.get(b"Resources") {
        Ok(r) => r,
        Err(_) => return Ok(out),
    };
    let res_dict = match doc.dereference(resources) {
        Ok((_, Object::Dictionary(d))) => d.clone(),
        Ok((_, Object::Reference(id))) => doc
            .get_dictionary(*id)
            .map_err(|e| Error::Pdf(e.to_string()))?
            .clone(),
        _ => return Ok(out),
    };
    let font_dict = match res_dict.get(b"Font") {
        Ok(f) => f,
        Err(_) => return Ok(out),
    };
    let fonts = match doc.dereference(font_dict) {
        Ok((_, Object::Dictionary(d))) => d.clone(),
        _ => font_dict.as_dict().ok().cloned().unwrap_or_default(),
    };
    for (key, value) in fonts.iter() {
        let name = String::from_utf8_lossy(key).into_owned();
        let Object::Reference(id) = value else {
            continue;
        };
        let id = *id;
        if let Ok(font) = PdfFont::load(doc, id, &name) {
            out.insert(name, font);
        }
    }
    Ok(out)
}

fn interpret_ops(
    doc: &Document,
    ctx: &mut ExtractCtx,
    gs: &mut GfxState,
    ops: &[Operation],
    resources: Option<&Object>,
    form_matrix: Matrix,
) -> Result<()> {
    let mut stack: Vec<GfxState> = Vec::new();
    for op in ops {
        match op.operator.as_str() {
            "q" => stack.push(gs.clone()),
            "Q" => {
                if let Some(prev) = stack.pop() {
                    *gs = prev;
                }
            }
            "cm" => {
                let m = matrix_from_ops(&op.operands);
                gs.ctm = m.concat(gs.ctm);
            }
            "w" => {
                if let Some(v) = op_num(&op.operands, 0) {
                    gs.line_width = v;
                }
            }
            "rg" => {
                gs.fill_space = PaintSpace::Rgb;
                gs.fill_rgb = rgb_ops(&op.operands);
            }
            "RG" => {
                gs.stroke_space = PaintSpace::Rgb;
                gs.stroke_rgb = rgb_ops(&op.operands);
            }
            "g" => {
                gs.fill_space = PaintSpace::Gray;
                let v = op_num(&op.operands, 0).unwrap_or(0.0);
                gs.fill_rgb = (v, v, v);
            }
            "G" => {
                gs.stroke_space = PaintSpace::Gray;
                let v = op_num(&op.operands, 0).unwrap_or(0.0);
                gs.stroke_rgb = (v, v, v);
            }
            "k" => {
                gs.fill_space = PaintSpace::Cmyk;
                gs.fill_rgb = cmyk_ops(&op.operands);
            }
            "K" => {
                gs.stroke_space = PaintSpace::Cmyk;
                gs.stroke_rgb = cmyk_ops(&op.operands);
            }
            "cs" => {
                if let Some(space) = resolve_paint_space(doc, &op.operands) {
                    gs.fill_space = space;
                }
            }
            "CS" => {
                if let Some(space) = resolve_paint_space(doc, &op.operands) {
                    gs.stroke_space = space;
                }
            }
            "sc" | "scn" => {
                gs.fill_rgb = paint_from_ops(&gs.fill_space, &op.operands);
            }
            "SC" | "SCN" => {
                gs.stroke_rgb = paint_from_ops(&gs.stroke_space, &op.operands);
            }
            "BT" => {
                gs.tm = Matrix::IDENTITY;
                gs.text_line_y = 0.0;
            }
            "ET" => {}
            "Tf" => {
                if let (Some(name), Some(size)) =
                    (op_name(&op.operands, 0), op_num(&op.operands, 1))
                {
                    gs.font_key = name;
                    gs.font_size = size;
                }
            }
            "Tm" => {
                gs.tm = matrix_from_ops(&op.operands);
            }
            "Td" | "TD" => {
                let tx = op_num(&op.operands, 0).unwrap_or(0.0);
                let ty = op_num(&op.operands, 1).unwrap_or(0.0);
                if op.operator == "TD" {
                    gs.text_line_y = ty;
                }
                gs.tm = Matrix::translate(tx, ty).concat(gs.tm);
            }
            "T*" => {
                gs.tm = Matrix::translate(0.0, -gs.text_line_y).concat(gs.tm);
            }
            "Tc" => gs.char_space = op_num(&op.operands, 0).unwrap_or(0.0),
            "Tw" => gs.word_space = op_num(&op.operands, 0).unwrap_or(0.0),
            "Tz" => gs.h_scale = op_num(&op.operands, 0).unwrap_or(100.0),
            "Tr" => gs.render_mode = op_num(&op.operands, 0).unwrap_or(0.0) as i32,
            "Tj" | "'" => {
                if op.operator == "'" {
                    gs.tm = Matrix::translate(0.0, -gs.text_line_y).concat(gs.tm);
                }
                if let Some(bytes) = op_string(&op.operands, 0) {
                    show_string(ctx, gs, &bytes, 0.0)?;
                }
            }
            "\"" => {
                let aw = op_num(&op.operands, 0).unwrap_or(0.0);
                let ac = op_num(&op.operands, 1).unwrap_or(0.0);
                gs.word_space = aw;
                gs.char_space = ac;
                gs.tm = Matrix::translate(0.0, -gs.text_line_y).concat(gs.tm);
                if let Some(bytes) = op_string(&op.operands, 2) {
                    show_string(ctx, gs, &bytes, 0.0)?;
                }
            }
            "TJ" => {
                if let Some(arr) = op.operands.first().and_then(|o| o.as_array().ok()) {
                    let mut offset = 0.0;
                    for item in arr {
                        match item {
                            Object::String(bytes, _) => {
                                show_string(ctx, gs, bytes, offset)?;
                                offset = 0.0;
                            }
                            Object::Integer(_) | Object::Real(_) => {
                                let adj = object_num(item).unwrap_or(0.0);
                                offset -= adj / 1000.0;
                            }
                            _ => {}
                        }
                    }
                }
            }
            "Do" => {
                if let (Some(name), res) = (op_name(&op.operands, 0), resources) {
                    handle_xobject(doc, ctx, gs, res, &name, form_matrix)?;
                }
            }
            "re" => {
                let x = op_num(&op.operands, 0).unwrap_or(0.0);
                let y = op_num(&op.operands, 1).unwrap_or(0.0);
                let w = op_num(&op.operands, 2).unwrap_or(0.0);
                let h = op_num(&op.operands, 3).unwrap_or(0.0);
                let p0 = transform(ctx, gs, x, y);
                let p1 = transform(ctx, gs, x + w, y);
                let p2 = transform(ctx, gs, x + w, y + h);
                let p3 = transform(ctx, gs, x, y + h);
                ctx.current_path.push(PathSegment::MoveTo(p0));
                ctx.current_path.push(PathSegment::LineTo(p1));
                ctx.current_path.push(PathSegment::LineTo(p2));
                ctx.current_path.push(PathSegment::LineTo(p3));
                ctx.current_path.push(PathSegment::Close);
                ctx.path_start = Some(p0);
                ctx.cur_point = Some(p3);
            }
            "W" | "W*" => {
                ctx.current_path.clear();
                ctx.path_start = None;
                ctx.cur_point = None;
            }
            "m" => {
                let x = op_num(&op.operands, 0).unwrap_or(0.0);
                let y = op_num(&op.operands, 1).unwrap_or(0.0);
                let p = transform(ctx, gs, x, y);
                ctx.current_path.push(PathSegment::MoveTo(p));
                ctx.cur_point = Some(p);
                ctx.path_start = Some(p);
            }
            "l" => {
                let x = op_num(&op.operands, 0).unwrap_or(0.0);
                let y = op_num(&op.operands, 1).unwrap_or(0.0);
                let p = transform(ctx, gs, x, y);
                ctx.current_path.push(PathSegment::LineTo(p));
                ctx.cur_point = Some(p);
            }
            "h" => {
                if let Some(start) = ctx.path_start {
                    ctx.current_path.push(PathSegment::LineTo(start));
                    ctx.cur_point = Some(start);
                }
                ctx.current_path.push(PathSegment::Close);
            }
            "f" | "F" | "f*" | "B" | "b" | "B*" | "b*" => {
                let even_odd = matches!(op.operator.as_str(), "f*" | "B*" | "b*");
                let _ = even_odd;
                let path = std::mem::take(&mut ctx.current_path);
                store_fill_path(ctx, gs, path);
                ctx.path_start = None;
                ctx.cur_point = None;
            }
            "S" | "s" => {
                let stroke = Some(RgbColor::from_pdf_rgb(
                    gs.stroke_rgb.0,
                    gs.stroke_rgb.1,
                    gs.stroke_rgb.2,
                ));
                let vp = VectorPath {
                    segments: std::mem::take(&mut ctx.current_path),
                    fill: None,
                    stroke,
                    line_width: gs.line_width,
                };
                if !vp.segments.is_empty() {
                    ctx.vectors.push(vp);
                }
                ctx.path_start = None;
                ctx.cur_point = None;
            }
            "n" => {
                ctx.current_path.clear();
                ctx.path_start = None;
                ctx.cur_point = None;
            }
            "BI" | "ID" | "EI" => {}
            _ => {}
        }
    }
    Ok(())
}

fn store_fill_path(ctx: &mut ExtractCtx, gs: &GfxState, path: Vec<PathSegment>) {
    if path.is_empty() {
        return;
    }
    let rgb = gs.fill_rgb;
    if rgb.0 > 0.99 && rgb.1 > 0.99 && rgb.2 > 0.99 {
        let (min_x, min_y, w, h) = path_bounds_segments(&path);
        if w >= ctx.page_w * 0.9 && h >= ctx.page_h * 0.9 {
            return;
        }
        let _ = min_x;
        let _ = min_y;
    }
    let fill = Some(RgbColor::from_pdf_rgb(rgb.0, rgb.1, rgb.2));
    ctx.vectors.push(VectorPath {
        segments: path,
        fill,
        stroke: None,
        line_width: gs.line_width,
    });
}

fn path_bounds_segments(path: &[PathSegment]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for seg in path {
        let pts = match seg {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => vec![*p],
            PathSegment::CurveTo(a, b, c) => vec![*a, *b, *c],
            PathSegment::Close => continue,
        };
        for p in pts {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    if min_x == f64::MAX {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    )
}

fn transform(ctx: &ExtractCtx, gs: &GfxState, x: f64, y: f64) -> PathPoint {
    let (ux, uy) = gs.ctm.transform(x, y);
    let (tx, ty) = ctx.pdf_y_to_top(ux, uy);
    PathPoint { x: tx, y: ty }
}

fn show_string(ctx: &mut ExtractCtx, gs: &mut GfxState, bytes: &[u8], x_offset: f64) -> Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let font = match gs.fonts.get(&gs.font_key) {
        Some(f) => f.clone(),
        None => return Ok(()),
    };
    let hidden = gs.render_mode == 3;
    let color = RgbColor::from_pdf_rgb(gs.fill_rgb.0, gs.fill_rgb.1, gs.fill_rgb.2);
    let hz = gs.h_scale / 100.0;
    let mut i = 0usize;
    let mut tj_extra = x_offset;
    while i < bytes.len() {
        let (glyph, step) = if font.is_cid_font() && i + 1 < bytes.len() {
            (&bytes[i..i + 2], 2)
        } else {
            (&bytes[i..i + 1], 1)
        };
        i += step;
        let text = font.decode_glyph(glyph);
        if text.is_empty() && !hidden {
            continue;
        }
        let combined = gs.tm.concat(gs.ctm);
        let (ux, uy) = combined.transform(tj_extra, 0.0);
        tj_extra = 0.0;
        let font_size = gs.font_size * combined.scale_x() * hz;
        let w_em = font.glyph_width_em(glyph);
        let gw = w_em / 1000.0 * font_size;
        let (top_x, top_y) = ctx.pdf_y_to_top(ux, uy);
        let ascent = font.ascent_pt(font_size);
        ctx.push_text_run(TextRun {
            text,
            x: top_x,
            y: top_y - ascent,
            width: gw,
            font_size,
            font_key: gs.font_key.clone(),
            font_family: font.display_family().to_owned(),
            bold: font.bold,
            italic: font.italic,
            color,
            char_spacing: gs.char_space,
            hidden,
            confidence: 1.0,
            highlight: false,
        });
        let mut adv = w_em / 1000.0 * hz + gs.char_space;
        let is_space = !font.is_cid_font() && glyph.first() == Some(&32);
        if is_space {
            adv += gs.word_space;
        }
        gs.tm = Matrix::translate(adv, 0.0).concat(gs.tm);
    }
    Ok(())
}

fn handle_xobject(
    doc: &Document,
    ctx: &mut ExtractCtx,
    gs: &GfxState,
    resources: Option<&Object>,
    name: &str,
    form_matrix: Matrix,
) -> Result<()> {
    let res = resources.ok_or_else(|| Error::Pdf("no resources".to_owned()))?;
    let res_dict = deref_dict(doc, res)?;
    let xobj = res_dict
        .get(b"XObject")
        .map_err(|_| Error::Pdf("no XObject".to_owned()))?;
    let xdict = deref_dict(doc, xobj)?;
    let key = name.as_bytes();
    let obj = xdict
        .get(key)
        .map_err(|_| Error::Pdf("xobject missing".to_owned()))?;
    let stream = match doc.dereference(obj) {
        Ok((_, Object::Stream(s))) => s.clone(),
        Err(_) => match obj {
            Object::Stream(s) => s.clone(),
            _ => return Ok(()),
        },
        Ok(_) => return Ok(()),
    };
    let subtype = stream
        .dict
        .get(b"Subtype")
        .ok()
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).into_owned())
        .unwrap_or_default();
    if subtype == "Image" {
        if let Some(img) = decode_image_stream(&stream)? {
            let (w, h) = image_size(&stream.dict)?;
            let m = gs.ctm;
            let corners = [
                m.transform(0.0, 0.0),
                m.transform(w, 0.0),
                m.transform(0.0, h),
                m.transform(w, h),
            ];
            let min_x = corners
                .iter()
                .map(|(x, _)| *x)
                .fold(f64::INFINITY, f64::min);
            let max_x = corners
                .iter()
                .map(|(x, _)| *x)
                .fold(f64::NEG_INFINITY, f64::max);
            let min_y = corners
                .iter()
                .map(|(_, y)| *y)
                .fold(f64::INFINITY, f64::min);
            let max_y = corners
                .iter()
                .map(|(_, y)| *y)
                .fold(f64::NEG_INFINITY, f64::max);
            let (tx, ty) = ctx.pdf_y_to_top(min_x, max_y);
            ctx.images.push(PlacedImage {
                bytes: img.0,
                kind: img.1,
                x: tx,
                y: ty,
                width: (max_x - min_x).abs(),
                height: (max_y - min_y).abs(),
                behind: false,
            });
        }
    } else if subtype == "Form" {
        let matrix = stream
            .dict
            .get(b"Matrix")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| matrix_from_array(a))
            .unwrap_or(Matrix::IDENTITY);
        let form_res = stream.dict.get(b"Resources").ok();
        let content = Content::decode(&stream.content).map_err(|e| Error::Pdf(e.to_string()))?;
        let mut sub_gs = gs.clone();
        if let Some(res) = form_res {
            if let Ok(extra) = fonts_from_resources(doc, res) {
                for (k, v) in extra {
                    sub_gs.fonts.insert(k, v);
                }
            }
        }
        interpret_ops(
            doc,
            ctx,
            &mut sub_gs,
            &content.operations,
            form_res,
            form_matrix.concat(matrix).concat(gs.ctm),
        )?;
    }
    Ok(())
}

fn image_size(dict: &lopdf::Dictionary) -> Result<(f64, f64)> {
    let w = dict_int(dict, b"Width") as f64;
    let h = dict_int(dict, b"Height") as f64;
    Ok((w, h))
}

fn decode_image_stream(stream: &Stream) -> Result<Option<(Vec<u8>, ImageKind)>> {
    let filters = stream_filters(&stream.dict);
    if filters.iter().any(|f| f == "DCTDecode") {
        return Ok(Some((stream.content.clone(), ImageKind::Jpeg)));
    }
    if filters.iter().any(|f| f == "FlateDecode") {
        let w = dict_int(&stream.dict, b"Width") as u32;
        let h = dict_int(&stream.dict, b"Height") as u32;
        let cs = stream
            .dict
            .get(b"ColorSpace")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_else(|| "DeviceRGB".to_owned());
        let bpc = dict_int(&stream.dict, b"BitsPerComponent") as u8;
        if bpc == 8 && (cs == "DeviceRGB" || cs == "DeviceGray") {
            let channels = if cs == "DeviceGray" { 1 } else { 3 };
            if let Some(png) = flate_to_png(&stream.content, w, h, channels) {
                return Ok(Some((png, ImageKind::Png)));
            }
        }
    }
    Ok(None)
}

fn flate_to_png(data: &[u8], w: u32, h: u32, channels: u32) -> Option<Vec<u8>> {
    let mut dict = lopdf::Dictionary::new();
    dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    let stream = Stream::new(dict, data.to_vec());
    let decoded = stream.decompressed_content().ok()?;
    let expected = (w * h * channels) as usize;
    if decoded.len() < expected {
        return None;
    }
    let mut buf = Vec::new();
    if channels == 1 {
        let gray = image::GrayImage::from_raw(w, h, decoded[..expected].to_vec())?;
        gray.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .ok()?;
    } else {
        let rgb = image::RgbImage::from_raw(w, h, decoded[..expected].to_vec())?;
        rgb.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .ok()?;
    }
    Some(buf)
}

fn stream_filters(dict: &lopdf::Dictionary) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(f) = dict.get(b"Filter") {
        if let Ok(name) = f.as_name() {
            out.push(String::from_utf8_lossy(name).into_owned());
        } else if let Ok(arr) = f.as_array() {
            for item in arr {
                if let Ok(name) = item.as_name() {
                    out.push(String::from_utf8_lossy(name).into_owned());
                }
            }
        }
    }
    out
}

fn deref_dict<'a>(doc: &'a Document, obj: &'a Object) -> Result<&'a lopdf::Dictionary> {
    if let Ok((_, Object::Dictionary(d))) = doc.dereference(obj) {
        return Ok(d);
    }
    obj.as_dict()
        .map_err(|_| Error::Pdf("expected dict".to_owned()))
}

fn dict_int(dict: &lopdf::Dictionary, key: &[u8]) -> i64 {
    dict.get(key)
        .ok()
        .and_then(|o| match o {
            Object::Integer(v) => Some(*v),
            Object::Real(v) => Some(f64::from(*v) as i64),
            _ => None,
        })
        .unwrap_or(0)
}

fn gap_needs_space(last: &TextRun, run: &TextRun, gap: f64, size: f64) -> bool {
    if gap > size * 0.06 {
        return true;
    }
    if gap <= size * 0.015 {
        return false;
    }
    let la = last.text.chars().last();
    let rb = run.text.chars().next();
    match (la, rb) {
        (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_alphabetic() => true,
        (Some(a), Some(b)) if a.is_ascii_alphabetic() && b.is_ascii_digit() => true,
        (Some('•' | '*' | '-' | '·'), Some(_)) => true,
        (Some(a), Some(b)) if a.is_alphanumeric() && b.is_alphanumeric() => gap > size * 0.02,
        _ => gap > size * 0.035,
    }
}

fn line_baseline(line: &[TextRun]) -> f64 {
    line.first().map(|r| r.y).unwrap_or(0.0)
}

fn merge_into_lines(raw: &[TextRun], fonts: &HashMap<String, PdfFont>) -> Vec<TextLine> {
    let visible: Vec<_> = raw
        .iter()
        .filter(|r| !r.hidden && !r.text.is_empty())
        .cloned()
        .collect();
    if visible.is_empty() {
        return Vec::new();
    }
    let mut work: Vec<TextRun> = visible;
    for run in &mut work {
        let snap = run.font_size * 0.15;
        if snap > 0.01 {
            run.y = (run.y / snap).round() * snap;
        }
    }
    let mut groups: Vec<Vec<TextRun>> = Vec::new();
    for run in work {
        if let Some(group) = groups.last_mut() {
            let base_y = line_baseline(group);
            let size = group
                .iter()
                .map(|r| r.font_size)
                .fold(0.0f64, f64::max)
                .max(run.font_size);
            if (run.y - base_y).abs() <= size * 0.35 {
                let Some(last) = group.last() else {
                    group.push(run.clone());
                    continue;
                };
                if last.text == "•" || last.text == "*" || last.text == "-" || last.text == "·" {
                    group.push(run);
                    continue;
                }
                let gap = run.x - (last.x + last.width);
                let space_w = fonts
                    .get(&last.font_key)
                    .map(|f| f.space_width(last.font_size))
                    .unwrap_or(last.font_size * 0.25);
                if gap > size * 0.35 {
                    groups.push(vec![run]);
                    continue;
                }
                if !gap_needs_space(last, &run, gap, size) && gap <= size * 0.06 {
                    let mut merged = last.clone();
                    merged.text.push_str(&run.text);
                    merged.width = run.x + run.width - merged.x;
                    let idx = group.len() - 1;
                    group[idx] = merged;
                } else if gap <= size * 0.55 {
                    let mut merged = last.clone();
                    merged.text.push(' ');
                    merged.width += space_w.min(gap.max(0.0));
                    merged.text.push_str(&run.text);
                    merged.width = run.x + run.width - merged.x;
                    let idx = group.len() - 1;
                    group[idx] = merged;
                } else if gap_needs_space(last, &run, gap, size) {
                    let mut nr = run;
                    nr.text = format!(" {}", nr.text);
                    nr.x -= space_w.min(gap.max(0.0));
                    group.push(nr);
                } else {
                    group.push(run);
                }
                continue;
            }
        }
        groups.push(vec![run]);
    }
    let mut lines = Vec::with_capacity(groups.len());
    for runs in groups {
        if runs.is_empty() {
            continue;
        }
        let x0 = runs.iter().map(|r| r.x).fold(f64::MAX, f64::min);
        let y0 = runs.iter().map(|r| r.y).fold(f64::MAX, f64::min);
        let x1 = runs.iter().map(|r| r.x + r.width).fold(f64::MIN, f64::max);
        let max_size = runs.iter().map(|r| r.font_size).fold(0.0f64, f64::max);
        let ascent = runs
            .iter()
            .filter_map(|r| fonts.get(&r.font_key).map(|f| f.ascent_pt(r.font_size)))
            .fold(max_size * 0.8, f64::max);
        lines.push(TextLine {
            runs,
            x: x0,
            y: y0,
            width: (x1 - x0).max(1.0) + 2.0,
            height: ascent.max(max_size * 0.5),
        });
    }
    lines.sort_by(|a, b| {
        let yc = a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal);
        if yc != std::cmp::Ordering::Equal {
            yc
        } else {
            a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    compress_adjacent_lines(lines)
}

fn compress_adjacent_lines(mut lines: Vec<TextLine>) -> Vec<TextLine> {
    if lines.is_empty() {
        return lines;
    }
    let mut out: Vec<TextLine> = Vec::new();
    let mut cur = lines.remove(0);
    for line in lines {
        let size = cur
            .runs
            .iter()
            .map(|r| r.font_size)
            .fold(0.0f64, f64::max)
            .max(line.runs.iter().map(|r| r.font_size).fold(0.0, f64::max));
        let same_row = (line.y - cur.y).abs() <= size * 0.25;
        let gap = line.x - (cur.x + cur.width);
        if same_row && gap < size * 2.0 {
            cur.runs.extend(line.runs);
            cur.width = (line.x + line.width - cur.x).max(cur.width);
        } else {
            out.push(cur);
            cur = line;
        }
    }
    out.push(cur);
    out
}

fn matrix_from_ops(ops: &[Object]) -> Matrix {
    let mut v = Vec::with_capacity(6);
    for o in ops {
        if let Ok(n) = object_num(o) {
            v.push(n);
        }
    }
    Matrix::from_pdf_operands(&v)
}

fn matrix_from_array(arr: &[Object]) -> Matrix {
    let mut v = Vec::with_capacity(6);
    for o in arr {
        if let Ok(n) = object_num(o) {
            v.push(n);
        }
    }
    Matrix::from_pdf_operands(&v)
}

fn op_num(ops: &[Object], i: usize) -> Option<f64> {
    ops.get(i).and_then(|o| object_num(o).ok())
}

fn op_name(ops: &[Object], i: usize) -> Option<String> {
    ops.get(i).and_then(|o| match o {
        Object::Name(n) => Some(String::from_utf8_lossy(n).into_owned()),
        _ => None,
    })
}

fn op_string(ops: &[Object], i: usize) -> Option<Vec<u8>> {
    ops.get(i).and_then(|o| match o {
        Object::String(b, _) => Some(b.clone()),
        _ => None,
    })
}

fn rgb_ops(ops: &[Object]) -> (f64, f64, f64) {
    (
        op_num(ops, 0).unwrap_or(0.0),
        op_num(ops, 1).unwrap_or(0.0),
        op_num(ops, 2).unwrap_or(0.0),
    )
}

fn cmyk_ops(ops: &[Object]) -> (f64, f64, f64) {
    cmyk_to_rgb(
        op_num(ops, 0).unwrap_or(0.0),
        op_num(ops, 1).unwrap_or(0.0),
        op_num(ops, 2).unwrap_or(0.0),
        op_num(ops, 3).unwrap_or(0.0),
    )
}

fn cmyk_to_rgb(c: f64, m: f64, y: f64, k: f64) -> (f64, f64, f64) {
    let r = (1.0 - c) * (1.0 - k);
    let g = (1.0 - m) * (1.0 - k);
    let b = (1.0 - y) * (1.0 - k);
    (r, g, b)
}

fn paint_from_ops(space: &PaintSpace, ops: &[Object]) -> (f64, f64, f64) {
    match space {
        PaintSpace::Gray => {
            let v = op_num(ops, 0).unwrap_or(0.0);
            (v, v, v)
        }
        PaintSpace::Rgb => rgb_ops(ops),
        PaintSpace::Cmyk => cmyk_ops(ops),
        PaintSpace::Indexed(palette) => {
            let idx = op_num(ops, 0).unwrap_or(0.0).round() as usize;
            palette.get(idx).copied().unwrap_or((0.0, 0.0, 0.0))
        }
    }
}

fn resolve_paint_space(doc: &Document, ops: &[Object]) -> Option<PaintSpace> {
    let first = ops.first()?;
    if let Ok(name) = first.as_name() {
        let n = String::from_utf8_lossy(name);
        return Some(match n.as_ref() {
            "DeviceGray" => PaintSpace::Gray,
            "DeviceRGB" => PaintSpace::Rgb,
            "DeviceCMYK" => PaintSpace::Cmyk,
            _ => PaintSpace::Rgb,
        });
    }
    let arr = first.as_array().ok()?;
    let kind = arr.first().and_then(|o| o.as_name().ok())?;
    let kind = String::from_utf8_lossy(kind);
    if kind == "ICCBased" {
        let n = icc_components(doc, arr.get(1)?)?;
        return Some(match n {
            1 => PaintSpace::Gray,
            3 => PaintSpace::Rgb,
            4 => PaintSpace::Cmyk,
            _ => PaintSpace::Rgb,
        });
    }
    if kind == "Indexed" {
        let base = resolve_paint_space(doc, std::slice::from_ref(arr.get(1)?))?;
        let palette = indexed_palette(doc, arr.get(2)?, &base)?;
        return Some(PaintSpace::Indexed(palette));
    }
    Some(PaintSpace::Rgb)
}

fn icc_components(doc: &Document, obj: &Object) -> Option<i64> {
    let stream = match doc.dereference(obj) {
        Ok((_, Object::Stream(s))) => s,
        Err(_) => obj.as_stream().ok()?,
        Ok(_) => return None,
    };
    stream.dict.get(b"N").ok().and_then(|o| o.as_i64().ok())
}

fn indexed_palette(
    doc: &Document,
    obj: &Object,
    base: &PaintSpace,
) -> Option<Vec<(f64, f64, f64)>> {
    let stream = match doc.dereference(obj) {
        Ok((_, Object::Stream(s))) => s,
        Err(_) => obj.as_stream().ok()?,
        Ok(_) => return None,
    };
    let hival = stream
        .dict
        .get(b"HiVal")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(255) as usize;
    let data = stream.decompressed_content().ok()?;
    let channels = match base {
        PaintSpace::Gray => 1,
        PaintSpace::Rgb => 3,
        PaintSpace::Cmyk => 4,
        PaintSpace::Indexed(_) => 3,
    };
    let mut out = Vec::with_capacity(hival + 1);
    for i in 0..=hival {
        let start = i * channels;
        if start + channels > data.len() {
            break;
        }
        let rgb = match channels {
            1 => {
                let v = data[start] as f64 / 255.0;
                (v, v, v)
            }
            3 => (
                data[start] as f64 / 255.0,
                data[start + 1] as f64 / 255.0,
                data[start + 2] as f64 / 255.0,
            ),
            4 => cmyk_to_rgb(
                data[start] as f64 / 255.0,
                data[start + 1] as f64 / 255.0,
                data[start + 2] as f64 / 255.0,
                data[start + 3] as f64 / 255.0,
            ),
            _ => (0.0, 0.0, 0.0),
        };
        out.push(rgb);
    }
    Some(out)
}

pub fn collect_fonts(
    pages: &[ExtractedPage],
    doc: &Document,
    page_ids: &[ObjectId],
) -> Vec<FontEmbed> {
    let mut seen = HashMap::new();
    let mut out = Vec::new();
    for &pid in page_ids {
        let page = match docsuite_pdf::page_attr::materialize_page_attributes(doc, pid) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let fonts = load_page_fonts(doc, &page).unwrap_or_default();
        for (key, font) in fonts {
            if seen.contains_key(&key) {
                continue;
            }
            if let Some(embed) = font.embed_info(&key) {
                seen.insert(key, ());
                out.push(embed);
            }
        }
    }
    let _ = pages;
    out
}

use crate::types::FontEmbed;

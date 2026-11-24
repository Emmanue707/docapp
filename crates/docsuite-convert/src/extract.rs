use std::collections::HashMap;

use docsuite_core::{Error, Result};
use lopdf::content::{Content, Operation};
use lopdf::{Document, Object, ObjectId, Stream};

use crate::fonts::PdfFont;
use crate::matrix::Matrix;
use crate::types::{
    ExtractedPage, ImageKind, PathPoint, PathSegment, PlacedImage, RgbColor, TextRun, VectorPath,
};

#[derive(Clone)]
struct GfxState {
    ctm: Matrix,
    fill_rgb: (f64, f64, f64),
    stroke_rgb: (f64, f64, f64),
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
    page_h: f64,
    text: Vec<TextRun>,
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
    ctx.text = merge_text_runs(&ctx.raw_runs, gs.fonts.clone());
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
                gs.fill_rgb = rgb_ops(&op.operands);
            }
            "RG" => {
                gs.stroke_rgb = rgb_ops(&op.operands);
            }
            "g" => {
                let v = op_num(&op.operands, 0).unwrap_or(0.0);
                gs.fill_rgb = (v, v, v);
            }
            "G" => {
                let v = op_num(&op.operands, 0).unwrap_or(0.0);
                gs.stroke_rgb = (v, v, v);
            }
            "k" => {
                gs.fill_rgb = cmyk_ops(&op.operands);
            }
            "K" => {
                gs.stroke_rgb = cmyk_ops(&op.operands);
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
                    show_string(doc, ctx, gs, &bytes, 0.0)?;
                }
            }
            "\"" => {
                let aw = op_num(&op.operands, 0).unwrap_or(0.0);
                let ac = op_num(&op.operands, 1).unwrap_or(0.0);
                gs.word_space = aw;
                gs.char_space = ac;
                gs.tm = Matrix::translate(0.0, -gs.text_line_y).concat(gs.tm);
                if let Some(bytes) = op_string(&op.operands, 2) {
                    show_string(doc, ctx, gs, &bytes, 0.0)?;
                }
            }
            "TJ" => {
                if let Some(arr) = op.operands.first().and_then(|o| o.as_array().ok()) {
                    let mut offset = 0.0;
                    for item in arr {
                        match item {
                            Object::String(bytes, _) => {
                                show_string(doc, ctx, gs, bytes, offset)?;
                                offset = 0.0;
                            }
                            Object::Integer(_) | Object::Real(_) => {
                                let adj = object_num(item).unwrap_or(0.0);
                                let font = gs.fonts.get(&gs.font_key);
                                let scale =
                                    font.map(|f| f.glyph_width(0, gs.font_size)).unwrap_or(1.0);
                                offset -= adj / 1000.0 * gs.font_size / scale.max(0.001);
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
                let p0 = transform(gs, x, y);
                let p1 = transform(gs, x + w, y);
                let p2 = transform(gs, x + w, y + h);
                let p3 = transform(gs, x, y + h);
                let segs = vec![
                    PathSegment::MoveTo(p0),
                    PathSegment::LineTo(p1),
                    PathSegment::LineTo(p2),
                    PathSegment::LineTo(p3),
                    PathSegment::Close,
                ];
                store_fill_path(ctx, gs, segs);
            }
            "m" => {
                let x = op_num(&op.operands, 0).unwrap_or(0.0);
                let y = op_num(&op.operands, 1).unwrap_or(0.0);
                let p = transform(gs, x, y);
                ctx.current_path.push(PathSegment::MoveTo(p));
                ctx.cur_point = Some(p);
                ctx.path_start = Some(p);
            }
            "l" => {
                let x = op_num(&op.operands, 0).unwrap_or(0.0);
                let y = op_num(&op.operands, 1).unwrap_or(0.0);
                let p = transform(gs, x, y);
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
    let fill = Some(RgbColor::from_pdf_rgb(
        gs.fill_rgb.0,
        gs.fill_rgb.1,
        gs.fill_rgb.2,
    ));
    ctx.vectors.push(VectorPath {
        segments: path,
        fill,
        stroke: None,
        line_width: gs.line_width,
    });
}

fn transform(gs: &GfxState, x: f64, y: f64) -> PathPoint {
    let (ux, uy) = gs.ctm.transform(x, y);
    PathPoint { x: ux, y: uy }
}

fn show_string(
    _doc: &Document,
    ctx: &mut ExtractCtx,
    gs: &mut GfxState,
    bytes: &[u8],
    x_offset: f64,
) -> Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let font = match gs.fonts.get(&gs.font_key) {
        Some(f) => f,
        None => return Ok(()),
    };
    let text = font.decode_string(bytes);
    if text.is_empty() {
        return Ok(());
    }
    let text_matrix = gs.tm;
    let combined = text_matrix.concat(gs.ctm);
    let (mut x, y) = combined.transform(0.0, 0.0);
    x += x_offset;
    let font_size = gs.font_size * combined.scale_x();
    let width = font.string_width(bytes, font_size);
    let (top_x, top_y) = ctx.pdf_y_to_top(x, y);
    let hidden = gs.render_mode == 3;
    let run = TextRun {
        text,
        x: top_x,
        y: top_y - font_size,
        width,
        font_size,
        font_key: gs.font_key.clone(),
        bold: font.bold,
        italic: font.italic,
        color: RgbColor::from_pdf_rgb(gs.fill_rgb.0, gs.fill_rgb.1, gs.fill_rgb.2),
        char_spacing: gs.char_space,
        hidden,
    };
    ctx.push_text_run(run);
    let adv = width / gs.font_size.max(0.001);
    gs.tm = Matrix::translate(adv, 0.0).concat(gs.tm);
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
            let (x0, y0) = m.transform(0.0, 0.0);
            let (x1, y1) = m.transform(w, 0.0);
            let (x2, y2) = m.transform(0.0, h);
            let min_x = x0.min(x1).min(x2);
            let max_x = x0.max(x1).max(x2);
            let min_y = y0.min(y1).min(y2);
            let max_y = y0.max(y1).max(y2);
            let (tx, ty) = ctx.pdf_y_to_top(min_x, max_y);
            ctx.images.push(PlacedImage {
                bytes: img.0,
                kind: img.1,
                x: tx,
                y: ty,
                width: (max_x - min_x).abs(),
                height: (max_y - min_y).abs(),
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

fn merge_text_runs(raw: &[TextRun], fonts: HashMap<String, PdfFont>) -> Vec<TextRun> {
    let visible: Vec<_> = raw.iter().filter(|r| !r.hidden).cloned().collect();
    if visible.is_empty() {
        return Vec::new();
    }
    let mut merged = Vec::new();
    let mut current = visible[0].clone();
    for run in visible.iter().skip(1) {
        let same_line = (run.y - current.y).abs() < 1.0;
        let same_style = run.font_key == current.font_key
            && (run.font_size - current.font_size).abs() < 0.5
            && run.color == current.color
            && run.bold == current.bold
            && run.italic == current.italic;
        let gap = run.x - (current.x + current.width);
        let em = current.font_size.max(1.0);
        if same_line && same_style && gap < em * 0.8 && gap > -1.0 {
            if gap > em * 0.25 {
                current.text.push(' ');
                current.width += em * 0.25;
            }
            current.text.push_str(&run.text);
            current.width = run.x + run.width - current.x;
        } else {
            merged.push(current);
            current = run.clone();
        }
    }
    merged.push(current);
    let _ = fonts;
    merged
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
    let c = op_num(ops, 0).unwrap_or(0.0);
    let m = op_num(ops, 1).unwrap_or(0.0);
    let y = op_num(ops, 2).unwrap_or(0.0);
    let k = op_num(ops, 3).unwrap_or(0.0);
    let r = (1.0 - c) * (1.0 - k);
    let g = (1.0 - m) * (1.0 - k);
    let b = (1.0 - y) * (1.0 - k);
    (r, g, b)
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

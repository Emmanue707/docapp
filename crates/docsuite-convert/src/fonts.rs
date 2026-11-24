use std::collections::HashMap;

use docsuite_core::{Error, Result};
use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::types::FontEmbed;

const WIN_ANSI: [u16; 256] = build_win_ansi();

const fn build_win_ansi() -> [u16; 256] {
    let mut t = [0u16; 256];
    let mut i = 0usize;
    while i < 256 {
        t[i] = i as u16;
        i += 1;
    }
    t[0x80] = 0x20AC;
    t[0x82] = 0x201A;
    t[0x83] = 0x0192;
    t[0x84] = 0x201E;
    t[0x85] = 0x2026;
    t[0x86] = 0x2020;
    t[0x87] = 0x2021;
    t[0x88] = 0x02C6;
    t[0x89] = 0x2030;
    t[0x8A] = 0x0160;
    t[0x8B] = 0x2039;
    t[0x8C] = 0x0152;
    t[0x8E] = 0x017D;
    t[0x91] = 0x2018;
    t[0x92] = 0x2019;
    t[0x93] = 0x201C;
    t[0x94] = 0x201D;
    t[0x95] = 0x2022;
    t[0x96] = 0x2013;
    t[0x97] = 0x2014;
    t[0x98] = 0x02DC;
    t[0x99] = 0x2122;
    t[0x9A] = 0x0161;
    t[0x9B] = 0x203A;
    t[0x9C] = 0x0153;
    t[0x9E] = 0x017E;
    t[0x9F] = 0x0178;
    t
}

static GLYPH_TO_UNICODE: &[(&str, char)] = &[
    ("space", ' '),
    ("A", 'A'),
    ("B", 'B'),
    ("C", 'C'),
    ("D", 'D'),
    ("E", 'E'),
    ("F", 'F'),
    ("G", 'G'),
    ("H", 'H'),
    ("I", 'I'),
    ("J", 'J'),
    ("K", 'K'),
    ("L", 'L'),
    ("M", 'M'),
    ("N", 'N'),
    ("O", 'O'),
    ("P", 'P'),
    ("Q", 'Q'),
    ("R", 'R'),
    ("S", 'S'),
    ("T", 'T'),
    ("U", 'U'),
    ("V", 'V'),
    ("W", 'W'),
    ("X", 'X'),
    ("Y", 'Y'),
    ("Z", 'Z'),
    ("a", 'a'),
    ("b", 'b'),
    ("c", 'c'),
    ("d", 'd'),
    ("e", 'e'),
    ("f", 'f'),
    ("g", 'g'),
    ("h", 'h'),
    ("i", 'i'),
    ("j", 'j'),
    ("k", 'k'),
    ("l", 'l'),
    ("m", 'm'),
    ("n", 'n'),
    ("o", 'o'),
    ("p", 'p'),
    ("q", 'q'),
    ("r", 'r'),
    ("s", 's'),
    ("t", 't'),
    ("u", 'u'),
    ("v", 'v'),
    ("w", 'w'),
    ("x", 'x'),
    ("y", 'y'),
    ("z", 'z'),
    ("zero", '0'),
    ("one", '1'),
    ("two", '2'),
    ("three", '3'),
    ("four", '4'),
    ("five", '5'),
    ("six", '6'),
    ("seven", '7'),
    ("eight", '8'),
    ("nine", '9'),
    ("period", '.'),
    ("comma", ','),
    ("hyphen", '-'),
];

pub fn strip_subset_prefix(name: &str) -> &str {
    if name.len() >= 7 {
        let prefix = &name[..6];
        if prefix
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && name.as_bytes().get(6) == Some(&b'+')
        {
            return &name[7..];
        }
    }
    name
}

fn trim_style_suffix(name: &str) -> String {
    let mut s = name.to_owned();
    for suffix in [
        "-BoldItalic",
        "-Bold",
        "-Italic",
        ",BoldItalic",
        ",Bold",
        ",Italic",
        "-BoldItalic",
        "-Italic",
        "-Regular",
    ] {
        if s.len() > suffix.len() {
            let tail = &s[s.len() - suffix.len()..];
            if tail.eq_ignore_ascii_case(suffix) {
                s.truncate(s.len() - suffix.len());
                break;
            }
        }
    }
    if s.ends_with("MT") || s.ends_with("PS") {
        let t = s.trim_end_matches("MT").trim_end_matches("PS");
        if !t.is_empty() {
            s = t.to_owned();
        }
    }
    s
}

pub fn family_name(base: &str) -> String {
    let stripped = strip_subset_prefix(base);
    let core = trim_style_suffix(stripped);
    let lower = core.to_ascii_lowercase();
    if lower.contains("liberationserif")
        || lower.contains("lmroman")
        || lower.starts_with("lmr")
        || lower.starts_with("cmr")
        || lower.starts_with("cmti")
        || lower.starts_with("cmmi")
        || lower.starts_with("cmsy")
        || lower.contains("nimbusrom")
        || lower.contains("latinmodernroman")
    {
        return "Times New Roman".to_owned();
    }
    if lower.contains("liberationsans") || lower.contains("lmss") || lower.contains("lmssq") {
        return "Arial".to_owned();
    }
    if lower.contains("liberationmono") || lower.contains("lmmono") || lower.contains("cmtt") {
        return "Courier New".to_owned();
    }
    if lower.contains("helvetica") || lower == "arial" || lower.contains("nimbussan") {
        return "Arial".to_owned();
    }
    if lower.contains("times") || lower.contains("nimbusrom") {
        return "Times New Roman".to_owned();
    }
    if lower.contains("courier") || lower.contains("nimbusmon") {
        return "Courier New".to_owned();
    }
    if lower == "symbol" {
        return "Symbol".to_owned();
    }
    core.to_owned()
}

pub fn style_from_name(name: &str) -> (bool, bool) {
    let n = name.to_ascii_lowercase();
    let bold = n.contains("bold") || n.contains("-bd") || n.ends_with("bd");
    let italic = n.contains("italic") || n.contains("oblique") || n.contains("-it");
    (bold, italic)
}

pub fn valid_xml_char(c: char) -> bool {
    let u = c as u32;
    matches!(c, '\t' | '\n' | '\r')
        || (c >= ' ' && c != '\u{FFFE}' && c != '\u{FFFF}' && !(0xD800..=0xDFFF).contains(&u))
}

#[derive(Clone)]
pub struct PdfFont {
    pub name: String,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    widths: Vec<f64>,
    first_char: u32,
    encoding: Vec<u16>,
    cid_map: HashMap<u32, char>,
    to_unicode: HashMap<u32, char>,
    is_cid: bool,
    cid_widths: HashMap<u32, f64>,
    cid_default_width: f64,
    default_width: f64,
    ascent_em: f64,
    space_em: f64,
    ttf: Option<Vec<u8>>,
    embed_family: Option<String>,
}

impl PdfFont {
    pub fn load(doc: &Document, font_ref: ObjectId, font_key: &str) -> Result<Self> {
        let dict = doc.get_dictionary(font_ref).map_err(pdf_err)?;
        let base = dict
            .get(b"BaseFont")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_else(|| font_key.to_owned());
        let family = family_name(&base);
        let (mut bold, mut italic) = style_from_name(&base);
        if let Ok(flags) = dict.get(b"Flags") {
            let fv = match flags {
                Object::Integer(v) => *v,
                _ => 0,
            };
            if fv & 0x40000 != 0 {
                bold = true;
            }
            if fv & 0x40 != 0 {
                italic = true;
            }
        }
        let subtype = dict
            .get(b"Subtype")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|n| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default();
        let mut cid_map = HashMap::new();
        let mut to_unicode = HashMap::new();
        let mut cid_widths = HashMap::new();
        let mut cid_default_width = 1000.0;
        let is_cid = subtype == "Type0";
        if is_cid {
            if let Ok(desc_ref) = dict.get(b"DescendantFonts") {
                if let Ok((_, Object::Array(arr))) = doc.dereference(desc_ref) {
                    if let Some(first) = arr.first() {
                        if let Ok(id) = first.as_reference() {
                            if let Ok(desc) = doc.get_dictionary(id) {
                                parse_cid_widths(desc, &mut cid_map);
                                let (dw, cw) = read_cid_w_table(desc);
                                cid_default_width = dw;
                                cid_widths = cw;
                            }
                        }
                    }
                }
            }
        }
        if let Ok(tu_ref) = dict.get(b"ToUnicode") {
            let stream_data = match doc.dereference(tu_ref) {
                Ok((_, Object::Stream(s))) => tu_stream_bytes(s),
                Err(_) => tu_ref.as_stream().ok().and_then(tu_stream_bytes),
                Ok((_, _)) => None,
            };
            if let Some(data) = stream_data {
                parse_to_unicode_cmap(&data, &mut to_unicode);
            }
        }
        let (first_char, widths) = read_widths(doc, dict)?;
        let encoding = build_encoding(doc, dict, &base)?;
        let (ttf, embed_family) = read_font_file2(doc, dict)?;
        let (ascent_em, space_em) = read_descriptor_metrics(doc, dict);
        Ok(Self {
            name: base,
            family,
            bold,
            italic,
            widths,
            first_char,
            encoding,
            cid_map,
            to_unicode,
            is_cid,
            cid_widths,
            cid_default_width,
            default_width: 500.0,
            ascent_em,
            space_em,
            ttf,
            embed_family,
        })
    }

    pub fn display_family(&self) -> &str {
        self.embed_family.as_deref().unwrap_or(&self.family)
    }

    pub fn is_cid_font(&self) -> bool {
        self.is_cid
    }

    pub fn ascent_pt(&self, font_size: f64) -> f64 {
        font_size * self.ascent_em
    }

    pub fn space_width(&self, font_size: f64) -> f64 {
        let em = if self.space_em > 0.0 {
            self.space_em
        } else {
            0.25
        };
        em * font_size
    }

    pub fn decode_string(&self, bytes: &[u8]) -> String {
        let mut out = String::new();
        if self.is_cid && bytes.len() >= 2 {
            let mut i = 0;
            while i + 1 < bytes.len() {
                let code = u32::from(bytes[i]) * 256 + u32::from(bytes[i + 1]);
                i += 2;
                let ch = self
                    .to_unicode
                    .get(&code)
                    .or_else(|| self.cid_map.get(&code));
                if let Some(c) = ch {
                    if valid_xml_char(*c) {
                        out.push(*c);
                    }
                }
            }
            return out;
        }
        let to_uni_only = !self.to_unicode.is_empty();
        for &b in bytes {
            let code = u32::from(b);
            let ch = if let Some(&ch) = self.to_unicode.get(&code) {
                Some(ch)
            } else if to_uni_only {
                None
            } else {
                let uni = self
                    .encoding
                    .get(b as usize)
                    .copied()
                    .unwrap_or(u16::from(b));
                if u32::from(uni) <= 0x10FFFF {
                    char::from_u32(u32::from(uni))
                } else {
                    None
                }
            };
            if let Some(c) = ch {
                if valid_xml_char(c) {
                    out.push(c);
                }
            }
        }
        out
    }

    pub fn glyph_width(&self, byte: u8, font_size: f64) -> f64 {
        let idx = byte as usize;
        let ci = self.first_char as usize;
        if idx >= ci {
            let wi = idx - ci;
            if let Some(&w) = self.widths.get(wi) {
                return w / 1000.0 * font_size;
            }
        }
        self.default_width / 1000.0 * font_size
    }

    pub fn cid_code_width_em(&self, code: u32) -> f64 {
        self.cid_widths
            .get(&code)
            .copied()
            .unwrap_or(self.cid_default_width)
    }

    pub fn glyph_width_em(&self, bytes: &[u8]) -> f64 {
        if self.is_cid && bytes.len() >= 2 {
            let code = u32::from(bytes[0]) * 256 + u32::from(bytes[1]);
            return self.cid_code_width_em(code);
        }
        if let Some(&b) = bytes.first() {
            return self.glyph_width(b, 1000.0);
        }
        self.default_width
    }

    pub fn decode_glyph(&self, bytes: &[u8]) -> String {
        if bytes.is_empty() {
            return String::new();
        }
        self.decode_string(bytes)
    }

    pub fn embed_info(&self, key: &str) -> Option<FontEmbed> {
        let _ = key;
        self.ttf.as_ref().map(|bytes| FontEmbed {
            pdf_name: self.name.clone(),
            family: self
                .embed_family
                .clone()
                .unwrap_or_else(|| self.family.clone()),
            ttf_bytes: bytes.clone(),
            bold: self.bold,
            italic: self.italic,
        })
    }
}

fn pdf_err(e: lopdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

fn tu_stream_bytes(s: &lopdf::Stream) -> Option<Vec<u8>> {
    s.decompressed_content()
        .ok()
        .or_else(|| Some(s.content.clone()))
}

fn read_widths(doc: &Document, dict: &Dictionary) -> Result<(u32, Vec<f64>)> {
    let first = dict
        .get(b"FirstChar")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0) as u32;
    let Ok(wobj) = dict.get(b"Widths") else {
        return Ok((first, Vec::new()));
    };
    let arr = match doc.dereference(wobj) {
        Ok((_, Object::Array(a))) => a.clone(),
        Ok((_, _)) => return Ok((first, Vec::new())),
        Err(_) => wobj.as_array().cloned().unwrap_or_default(),
    };
    let mut widths = Vec::with_capacity(arr.len());
    for item in arr {
        let w = match item {
            Object::Integer(v) => v as f64,
            Object::Real(v) => f64::from(v),
            _ => 500.0,
        };
        widths.push(w);
    }
    Ok((first, widths))
}

fn build_encoding(doc: &Document, dict: &Dictionary, base: &str) -> Result<Vec<u16>> {
    let mut table = WIN_ANSI.to_vec();
    if let Ok(enc) = dict.get(b"Encoding") {
        match enc {
            Object::Name(n) if n.as_slice() == b"WinAnsiEncoding" => {}
            Object::Name(n) if n.as_slice() == b"MacRomanEncoding" => {
                table = WIN_ANSI.to_vec();
            }
            Object::Reference(id) => {
                let d = doc.get_dictionary(*id).map_err(pdf_err)?;
                if let Ok(diff) = d.get(b"Differences") {
                    if let Ok(arr) = diff.as_array() {
                        let mut code = 0u32;
                        for item in arr {
                            match item {
                                Object::Integer(c) => code = *c as u32,
                                Object::Name(glyph) => {
                                    let g = String::from_utf8_lossy(glyph);
                                    if let Some((_, ch)) =
                                        GLYPH_TO_UNICODE.iter().find(|(name, _)| *name == g)
                                    {
                                        if (code as usize) < table.len() {
                                            table[code as usize] = *ch as u16;
                                        }
                                    }
                                    code += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    } else {
        let lower = base.to_ascii_lowercase();
        if lower.contains("times") || lower.contains("courier") || lower.contains("helvetica") {
            let _ = lower;
        }
    }
    Ok(table)
}

fn parse_to_unicode_cmap(data: &[u8], map: &mut HashMap<u32, char>) {
    let text = String::from_utf8_lossy(data);
    parse_bfchar(&text, map);
    parse_bfrange(&text, map);
}

fn parse_bfchar(text: &str, map: &mut HashMap<u32, char>) {
    let mut in_bf = false;
    for line in text.lines() {
        let t = line.trim();
        if t.contains("beginbfchar") {
            in_bf = true;
            continue;
        }
        if t.contains("endbfchar") {
            in_bf = false;
            continue;
        }
        if in_bf && t.starts_with('<') {
            if let Some((src, dst)) = t.split_once('>') {
                let src_hex = src.trim_start_matches('<');
                let dst_part = dst.trim().trim_start_matches('<').trim_end_matches('>');
                if let (Ok(code), Some(ch)) =
                    (u32::from_str_radix(src_hex, 16), hex_to_char(dst_part))
                {
                    map.insert(code, ch);
                }
            }
        }
    }
}

fn parse_bfrange(text: &str, map: &mut HashMap<u32, char>) {
    let mut in_bf = false;
    for line in text.lines() {
        let t = line.trim();
        if t.contains("beginbfrange") {
            in_bf = true;
            continue;
        }
        if t.contains("endbfrange") {
            in_bf = false;
            continue;
        }
        if !in_bf || !t.starts_with('<') {
            continue;
        }
        let parts: Vec<&str> = t.split_whitespace().collect();
        if parts.len() >= 3 {
            let start = parts[0].trim_matches(|c| c == '<' || c == '>');
            let end = parts[1].trim_matches(|c| c == '<' || c == '>');
            let dst = parts[2].trim_matches(|c| c == '<' || c == '>');
            if let (Ok(s), Ok(e)) = (u32::from_str_radix(start, 16), u32::from_str_radix(end, 16)) {
                if dst.len() >= 4 {
                    if let Ok(base) = u32::from_str_radix(&dst[..4], 16) {
                        for (i, code) in (s..=e).enumerate() {
                            if let Some(ch) = char::from_u32(base + i as u32) {
                                map.insert(code, ch);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn hex_to_char(hex: &str) -> Option<char> {
    let h = hex.trim();
    if h.len() >= 4 {
        let code = u32::from_str_radix(&h[h.len() - 4..], 16).ok()?;
        char::from_u32(code)
    } else {
        None
    }
}

fn read_cid_w_table(desc: &Dictionary) -> (f64, HashMap<u32, f64>) {
    let dw = desc
        .get(b"DW")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(1000) as f64;
    let mut widths = HashMap::new();
    if let Ok(wobj) = desc.get(b"W") {
        if let Ok(arr) = wobj.as_array() {
            parse_w_array(arr, &mut widths);
        }
    }
    (dw, widths)
}

fn pdf_real(obj: &Object) -> Option<f64> {
    match obj {
        Object::Real(v) => Some(f64::from(*v)),
        Object::Integer(v) => Some(*v as f64),
        _ => None,
    }
}

fn parse_w_array(arr: &[Object], out: &mut HashMap<u32, f64>) {
    let mut i = 0;
    while i < arr.len() {
        let cid = match &arr[i] {
            Object::Integer(v) => *v as u32,
            Object::Real(v) => f64::from(*v) as u32,
            _ => {
                i += 1;
                continue;
            }
        };
        i += 1;
        if i >= arr.len() {
            break;
        }
        if let Ok(sub) = arr[i].as_array() {
            for (k, item) in sub.iter().enumerate() {
                if let Ok(w) = item.as_i64() {
                    out.insert(cid + k as u32, w as f64);
                } else if let Some(w) = pdf_real(item) {
                    out.insert(cid + k as u32, w);
                }
            }
            i += 1;
        } else if let Ok(end) = arr[i].as_i64() {
            if i + 1 < arr.len() {
                if let Ok(w) = arr[i + 1].as_i64() {
                    for c in cid..=(end as u32) {
                        out.insert(c, w as f64);
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }
        } else if let Ok(w) = arr[i].as_i64() {
            out.insert(cid, w as f64);
            i += 1;
        } else if let Some(w) = pdf_real(&arr[i]) {
            out.insert(cid, w);
            i += 1;
        } else {
            i += 1;
        }
    }
}

fn parse_cid_widths(desc: &Dictionary, map: &mut HashMap<u32, char>) {
    if let Ok(ci) = desc.get(b"CIDSystemInfo") {
        if let Ok(d) = ci.as_dict() {
            if let Ok(ordering) = d.get(b"Ordering") {
                if let Ok(name) = ordering.as_name() {
                    let ord = String::from_utf8_lossy(name);
                    if ord == "Identity" {
                        let _ = map;
                    }
                }
            }
        }
    }
}

fn read_descriptor_metrics(doc: &Document, dict: &Dictionary) -> (f64, f64) {
    let desc = match dict.get(b"FontDescriptor") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).ok(),
        Ok(Object::Dictionary(d)) => Some(d),
        _ => None,
    };
    let Some(desc) = desc else {
        return (0.8, 0.25);
    };
    let ascent = desc
        .get(b"Ascent")
        .ok()
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(800) as f64
        / 1000.0;
    let mut space_em = 0.25;
    if let Ok((first, widths)) = read_widths(doc, dict) {
        let sp = 32u32;
        if sp >= first {
            let wi = (sp - first) as usize;
            if let Some(&w) = widths.get(wi) {
                space_em = w / 1000.0;
            }
        }
    }
    (ascent.max(0.5), space_em.max(0.1))
}

fn read_embed_family_from_ttf(data: &[u8]) -> Option<String> {
    if data.len() < 12 {
        return None;
    }
    let num_tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let mut name_offset = 0usize;
    let mut name_len = 0usize;
    for i in 0..num_tables {
        let off = 12 + i * 16;
        if off + 16 > data.len() {
            break;
        }
        if &data[off..off + 4] == b"name" {
            name_offset =
                u32::from_be_bytes([data[off + 8], data[off + 9], data[off + 10], data[off + 11]])
                    as usize;
            name_len = u32::from_be_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]) as usize;
            break;
        }
    }
    if name_offset + 6 > data.len() || name_len == 0 {
        return None;
    }
    let count = u16::from_be_bytes([data[name_offset + 2], data[name_offset + 3]]) as usize;
    let string_offset =
        name_offset + u16::from_be_bytes([data[name_offset + 4], data[name_offset + 5]]) as usize;
    for i in 0..count {
        let rec = name_offset + 6 + i * 12;
        if rec + 12 > data.len() {
            break;
        }
        let name_id = u16::from_be_bytes([data[rec + 6], data[rec + 7]]);
        if name_id != 1 && name_id != 4 {
            continue;
        }
        let length = u16::from_be_bytes([data[rec + 8], data[rec + 9]]) as usize;
        let off = u16::from_be_bytes([data[rec + 10], data[rec + 11]]) as usize;
        let start = string_offset + off;
        if start + length > data.len() {
            continue;
        }
        let slice = &data[start..start + length];
        let is_utf16 = data[rec + 2] == 0 && data[rec + 3] == 3;
        if is_utf16 && length >= 2 {
            let mut s = String::new();
            let mut j = 0;
            while j + 1 < length {
                let u = u16::from_be_bytes([slice[j], slice[j + 1]]);
                if let Some(c) = char::from_u32(u32::from(u)) {
                    s.push(c);
                }
                j += 2;
            }
            if !s.is_empty() {
                return Some(s);
            }
        } else if let Ok(s) = std::str::from_utf8(slice) {
            return Some(s.to_owned());
        }
    }
    None
}

fn read_font_file2(doc: &Document, dict: &Dictionary) -> Result<(Option<Vec<u8>>, Option<String>)> {
    let desc = match dict.get(b"FontDescriptor") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).map_err(pdf_err)?,
        Ok(Object::Dictionary(d)) => d,
        _ => return Ok((None, None)),
    };
    if let Ok(f) = desc.get(b"FontFile2") {
        if let Ok((_, Object::Stream(s))) = doc.dereference(f) {
            let fam = read_embed_family_from_ttf(&s.content);
            return Ok((Some(s.content.clone()), fam));
        }
    }
    if desc.get(b"FontFile3").is_ok() || desc.get(b"FontFile").is_ok() {
        return Ok((None, None));
    }
    Ok((None, None))
}

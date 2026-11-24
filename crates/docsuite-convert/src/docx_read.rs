use std::collections::HashMap;
use std::io::Cursor;

use docsuite_core::{Error, Result};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::doc::{
    Align, Block, Document, ImageBlock, ListKind, ListStyle, Paragraph, ParagraphProps,
    ParagraphStyle, Run, Table, TableCell, TableRow,
};
use crate::ooxf::{collect_rels, read_zip_bytes};
use crate::types::{ImageKind, RgbColor};
use crate::xml_util::{end_local, start_local};

pub fn read_docx_package(pkg: &[u8]) -> Result<Document> {
    let doc_xml = read_zip_bytes(pkg, "word/document.xml")?;
    let styles_xml = read_zip_bytes(pkg, "word/styles.xml").ok();
    let rels = collect_rels(pkg, "word/_rels/document.xml.rels").unwrap_or_default();
    let styles = styles_xml
        .as_ref()
        .map(|s| parse_styles(s))
        .unwrap_or_default();
    let mut doc = parse_document_body(&doc_xml, &styles, &rels)?;
    doc = resolve_images(pkg, doc, &rels)?;
    Ok(doc)
}

#[derive(Default, Clone)]
struct StyleDef {
    based_on: Option<String>,
    outline: Option<u8>,
    bold: bool,
    italic: bool,
    size_half: Option<i64>,
}

fn parse_styles(xml: &[u8]) -> HashMap<String, StyleDef> {
    let mut map = HashMap::new();
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current_id: Option<String> = None;
    let mut current = StyleDef::default();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = start_local(&e);
                if name == b"style" {
                    current_id = attr_string(&e, b"w:styleId");
                    current = StyleDef::default();
                } else if name == b"basedOn" {
                    current.based_on = attr_string(&e, b"w:val");
                } else if name == b"outlineLvl" {
                    if let Some(v) = attr_string(&e, b"w:val")
                        && let Ok(n) = v.parse::<u8>()
                    {
                        current.outline = Some(n.saturating_add(1));
                    }
                } else if name == b"b" {
                    current.bold = true;
                } else if name == b"i" {
                    current.italic = true;
                } else if name == b"sz" {
                    current.size_half = attr_string(&e, b"w:val").and_then(|v| v.parse().ok());
                }
            }
            Ok(Event::End(e)) if end_local(&e) == b"style" => {
                if let Some(id) = current_id.take() {
                    map.insert(id, current.clone());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

fn resolve_style(styles: &HashMap<String, StyleDef>, id: &str) -> StyleDef {
    let mut def = styles.get(id).cloned().unwrap_or_default();
    if let Some(base) = &def.based_on {
        let parent = resolve_style(styles, base);
        if def.outline.is_none() {
            def.outline = parent.outline;
        }
        if !def.bold {
            def.bold = parent.bold;
        }
        if !def.italic {
            def.italic = parent.italic;
        }
        if def.size_half.is_none() {
            def.size_half = parent.size_half;
        }
    }
    def
}

fn parse_document_body(
    xml: &[u8],
    styles: &HashMap<String, StyleDef>,
    rels: &HashMap<String, String>,
) -> Result<Document> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut doc = Document::default();
    let mut run_stack: Vec<Run> = Vec::new();
    let mut table_stack: Vec<Table> = Vec::new();
    let mut row_cells: Vec<TableCell> = Vec::new();
    let mut cell_blocks: Vec<Block> = Vec::new();
    let mut in_tbl = false;
    let mut current_rpr = Run::default();
    let mut current_ppr = ParagraphProps::default();
    let mut current_pstyle = ParagraphStyle::Normal;
    let mut text_buf = String::new();
    let mut rid_for_image: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = start_local(&e);
                match name {
                    b"p" if !in_tbl => {
                        current_ppr = ParagraphProps::default();
                        current_pstyle = ParagraphStyle::Normal;
                        text_buf.clear();
                    }
                    b"p" if in_tbl => {
                        current_ppr = ParagraphProps::default();
                        current_pstyle = ParagraphStyle::Normal;
                    }
                    b"pPr" => {}
                    b"pStyle" => {
                        if let Some(v) = attr_string(&e, b"w:val") {
                            if v.starts_with("Heading") {
                                if let Some(d) = v.strip_prefix("Heading")
                                    && let Ok(l) = d.parse::<u8>()
                                {
                                    current_pstyle = ParagraphStyle::Heading(l);
                                }
                            } else if styles.contains_key(&v) {
                                let resolved = resolve_style(styles, &v);
                                if let Some(outline) = resolved.outline {
                                    current_pstyle = ParagraphStyle::Heading(outline);
                                }
                            }
                        }
                    }
                    b"jc" => {
                        if let Some(v) = attr_string(&e, b"w:val") {
                            current_ppr.align = Some(match v.as_str() {
                                "center" => Align::Center,
                                "right" => Align::Right,
                                "both" => Align::Justify,
                                _ => Align::Left,
                            });
                        }
                    }
                    b"numPr" => {}
                    b"numId" => {
                        if let Some(v) = attr_string(&e, b"w:val") {
                            let kind = if v == "2" {
                                ListKind::Bullet
                            } else {
                                ListKind::Ordered
                            };
                            current_pstyle = ParagraphStyle::List(ListStyle {
                                kind,
                                level: 0,
                                number_format: "%1.".to_owned(),
                            });
                        }
                    }
                    b"ilvl" => {
                        if let ParagraphStyle::List(ref mut ls) = current_pstyle {
                            if let Some(v) = attr_string(&e, b"w:val")
                                && let Ok(n) = v.parse()
                            {
                                ls.level = n;
                            }
                        }
                    }
                    b"tbl" => {
                        in_tbl = true;
                        table_stack.push(Table {
                            col_widths_pt: Vec::new(),
                            rows: Vec::new(),
                            borders: true,
                        });
                    }
                    b"tr" => {
                        row_cells.clear();
                    }
                    b"tc" => {
                        cell_blocks.clear();
                    }
                    b"r" => {
                        current_rpr = Run::default();
                    }
                    b"rPr" => {}
                    b"b" => current_rpr.bold = true,
                    b"i" => current_rpr.italic = true,
                    b"u" => current_rpr.underline = true,
                    b"strike" => current_rpr.strike = true,
                    b"sz" => {
                        if let Some(v) = attr_string(&e, b"w:val")
                            && let Ok(h) = v.parse::<f64>()
                        {
                            current_rpr.size_pt = Some(h / 2.0);
                        }
                    }
                    b"color" => {
                        if let Some(v) = attr_string(&e, b"w:val")
                            && v.len() == 6
                            && let (Ok(r), Ok(g), Ok(b)) = (
                                u8::from_str_radix(&v[0..2], 16),
                                u8::from_str_radix(&v[2..4], 16),
                                u8::from_str_radix(&v[4..6], 16),
                            )
                        {
                            current_rpr.color = Some(RgbColor { r, g, b });
                        }
                    }
                    b"highlight" => current_rpr.highlight = true,
                    b"br" => {
                        if attr_string(&e, b"w:type").as_deref() == Some("page") {
                            doc.blocks.push(Block::PageBreak);
                        }
                    }
                    b"blip" => {
                        rid_for_image = attr_string(&e, b"r:embed");
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let t = e.unescape().unwrap_or_default();
                if !t.as_ref().is_empty() {
                    text_buf.push_str(&t);
                }
            }
            Ok(Event::End(e)) => {
                let name = end_local(&e);
                match name {
                    b"r" => {
                        if !text_buf.is_empty() {
                            let mut run = current_rpr.clone();
                            run.text = std::mem::take(&mut text_buf);
                            run_stack.push(run);
                        } else if !current_rpr.bold
                            && !current_rpr.italic
                            && current_rpr.size_pt.is_some()
                        {
                            run_stack.push(current_rpr.clone());
                        }
                        current_rpr = Run::default();
                    }
                    b"p" => {
                        let para = Paragraph {
                            style: current_pstyle.clone(),
                            props: current_ppr.clone(),
                            runs: if run_stack.is_empty() && !text_buf.is_empty() {
                                vec![Run {
                                    text: std::mem::take(&mut text_buf),
                                    ..Run::default()
                                }]
                            } else {
                                std::mem::take(&mut run_stack)
                            },
                        };
                        if in_tbl {
                            if !para.runs.is_empty() || matches!(para.style, ParagraphStyle::Normal)
                            {
                                cell_blocks.push(Block::Paragraph(para));
                            }
                        } else {
                            doc.blocks.push(Block::Paragraph(para));
                        }
                        if let Some(rid) = rid_for_image.take() {
                            if let Some(target) = rels.get(&rid) {
                                let _ = target;
                            }
                        }
                    }
                    b"tc" => {
                        row_cells.push(TableCell {
                            blocks: std::mem::take(&mut cell_blocks),
                            colspan: 1,
                            rowspan: 1,
                            shading: None,
                        });
                    }
                    b"tr" => {
                        if let Some(tbl) = table_stack.last_mut() {
                            tbl.rows.push(TableRow {
                                cells: std::mem::take(&mut row_cells),
                            });
                        }
                    }
                    b"tbl" => {
                        in_tbl = false;
                        if let Some(tbl) = table_stack.pop() {
                            doc.blocks.push(Block::Table(tbl));
                        }
                    }
                    b"drawing" => {
                        if let Some(rid) = rid_for_image.take()
                            && let Some(target) = rels.get(&rid)
                        {
                            let pending = (rid, target.clone());
                            let _ = pending;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Parse(e.to_string())),
            _ => {}
        }
        buf.clear();
    }
    let _ = rels;
    Ok(doc)
}

fn resolve_images(
    pkg: &[u8],
    mut doc: Document,
    rels: &HashMap<String, String>,
) -> Result<Document> {
    let media: HashMap<String, Vec<u8>> = rels
        .iter()
        .filter_map(|(rid, target)| {
            let path = format!("word/{target}");
            read_zip_bytes(pkg, &path).ok().map(|b| (rid.clone(), b))
        })
        .collect();
    for block in &mut doc.blocks {
        attach_block_images(block, &media);
    }
    Ok(doc)
}

fn attach_block_images(block: &mut Block, media: &HashMap<String, Vec<u8>>) {
    let _ = media;
    if let Block::Table(t) = block {
        for row in &mut t.rows {
            for cell in &mut row.cells {
                for b in &mut cell.blocks {
                    attach_block_images(b, media);
                }
            }
        }
    }
}

fn attr_string(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
}

use std::collections::HashMap;
use std::io::Cursor;

use docsuite_core::{Error, Result};
use quick_xml::Reader;
use quick_xml::events::Event;

use crate::doc::{
    Block, Document, ImageBlock, ListKind, ListStyle, Paragraph, ParagraphProps, ParagraphStyle,
    Run, Table, TableCell, TableRow,
};
use crate::ooxf::read_zip_bytes;
use crate::types::{ImageKind, RgbColor};
use crate::xml_util::{end_local, start_local};

pub fn read_odt_package(pkg: &[u8]) -> Result<Document> {
    let content = read_zip_bytes(pkg, "content.xml")?;
    let styles = read_zip_bytes(pkg, "styles.xml").ok();
    let style_parents = styles
        .as_ref()
        .map(|s| parse_style_parents(s))
        .unwrap_or_default();
    parse_content(&content, &style_parents, pkg)
}

fn parse_style_parents(xml: &[u8]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current: Option<String> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = start_local(&e);
                if name == b"style" || name == b"default-style" {
                    current = attr_val(&e, b"style:name");
                } else if name == b"parent-style-name" {
                    if let (Some(id), Some(p)) =
                        (current.clone(), attr_val(&e, b"style:parent-style-name"))
                    {
                        map.insert(id, p);
                    }
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

fn parse_content(xml: &[u8], _parents: &HashMap<String, String>, pkg: &[u8]) -> Result<Document> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut doc = Document::default();
    let mut run = Run::default();
    let mut runs: Vec<Run> = Vec::new();
    let mut para_style = ParagraphStyle::Normal;
    let mut props = ParagraphProps::default();
    let mut table: Option<Table> = None;
    let mut in_table = false;
    let mut row_cells: Vec<TableCell> = Vec::new();
    let mut cell_blocks: Vec<Block> = Vec::new();
    let mut list_level = 0u32;
    let mut pending_href: Option<String> = None;
    let mut image_href: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name = start_local(&e);
                match name {
                    b"p" => {
                        props = ParagraphProps::default();
                        para_style = ParagraphStyle::Normal;
                        if let Some(style) = attr_val(&e, b"text:style-name") {
                            if style.starts_with("Heading") {
                                if let Some(n) = style.strip_prefix("Heading")
                                    && let Ok(l) = n.parse::<u8>()
                                {
                                    para_style = ParagraphStyle::Heading(l);
                                }
                            }
                        }
                    }
                    b"h" => {
                        let level = attr_val(&e, b"text:outline-level")
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(1);
                        para_style = ParagraphStyle::Heading(level);
                    }
                    b"list" => {
                        list_level = 0;
                    }
                    b"list-item" => {}
                    b"list-level-style-number" => {}
                    b"table" => {
                        in_table = true;
                        table = Some(Table {
                            col_widths_pt: Vec::new(),
                            rows: Vec::new(),
                            borders: true,
                        });
                    }
                    b"table-row" => row_cells.clear(),
                    b"table-cell" => cell_blocks.clear(),
                    b"span" => {
                        run = Run::default();
                        if attr_val(&e, b"text:style-name")
                            .map(|s| s.contains("Bold"))
                            .unwrap_or(false)
                        {
                            run.bold = true;
                        }
                        if attr_val(&e, b"text:style-name")
                            .map(|s| s.contains("Italic"))
                            .unwrap_or(false)
                        {
                            run.italic = true;
                        }
                    }
                    b"a" => pending_href = attr_val(&e, b"xlink:href"),
                    b"frame" | b"image" if image_href.is_none() => {
                        image_href = attr_val(&e, b"xlink:href");
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                let t = e.unescape().unwrap_or_default();
                if !t.is_empty() {
                    run.text.push_str(&t);
                    if let Some(h) = pending_href.take() {
                        run.link = Some(h);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name = end_local(&e);
                match name {
                    b"span" | b"a" => {
                        if !run.text.is_empty() {
                            runs.push(std::mem::take(&mut run));
                        }
                    }
                    b"p" | b"h" => {
                        if !run.text.is_empty() {
                            runs.push(std::mem::take(&mut run));
                        }
                        let para = Paragraph {
                            style: para_style.clone(),
                            props: props.clone(),
                            runs: std::mem::take(&mut runs),
                        };
                        if in_table {
                            cell_blocks.push(Block::Paragraph(para));
                        } else {
                            if matches!(para_style, ParagraphStyle::Normal) && list_level > 0 {
                                let mut p = para;
                                p.style = ParagraphStyle::List(ListStyle {
                                    kind: ListKind::Bullet,
                                    level: list_level.saturating_sub(1),
                                    number_format: "•".to_owned(),
                                });
                                doc.blocks.push(Block::Paragraph(p));
                            } else {
                                doc.blocks.push(Block::Paragraph(para));
                            }
                        }
                        if let Some(href) = image_href.take() {
                            if let Some(img) = load_odt_image(pkg, &href) {
                                if in_table {
                                    cell_blocks.push(Block::Image(img));
                                } else {
                                    doc.blocks.push(Block::Image(img));
                                }
                            }
                        }
                    }
                    b"table-cell" => {
                        row_cells.push(TableCell {
                            blocks: std::mem::take(&mut cell_blocks),
                            colspan: 1,
                            rowspan: 1,
                            shading: None,
                        });
                    }
                    b"table-row" => {
                        if let Some(ref mut tbl) = table {
                            tbl.rows.push(TableRow {
                                cells: std::mem::take(&mut row_cells),
                            });
                        }
                    }
                    b"table" => {
                        in_table = false;
                        if let Some(tbl) = table.take() {
                            doc.blocks.push(Block::Table(tbl));
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
    Ok(doc)
}

fn load_odt_image(pkg: &[u8], href: &str) -> Option<ImageBlock> {
    let path = href.trim_start_matches('/');
    let bytes = read_zip_bytes(pkg, path).ok()?;
    let kind = if path.ends_with(".png") {
        ImageKind::Png
    } else {
        ImageKind::Jpeg
    };
    Some(ImageBlock {
        bytes,
        kind,
        width_pt: 300.0,
        height_pt: 200.0,
    })
}

fn attr_val(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
}

use serde::Serialize;

use super::{
    Align, Block, Document, HeaderFooter, LineSpacing, ListKind, Orientation, ParagraphProps,
    ParagraphStyle, Run,
};
use crate::types::{ImageKind, RgbColor};

#[derive(Serialize)]
struct DocJson {
    page: PageJson,
    header: Option<HeaderJson>,
    footer: Option<HeaderJson>,
    blocks: Vec<BlockJson>,
}

#[derive(Serialize)]
struct PageJson {
    width_pt: f64,
    height_pt: f64,
    margin_top_pt: f64,
    margin_right_pt: f64,
    margin_bottom_pt: f64,
    margin_left_pt: f64,
    orientation: String,
}

#[derive(Serialize)]
struct HeaderJson {
    runs: Vec<RunJson>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BlockJson {
    Paragraph(ParagraphJson),
    Table(TableJson),
    Image(ImageJson),
    PageBreak,
}

#[derive(Serialize)]
struct ParagraphJson {
    style: ParaStyleJson,
    props: ParaPropsJson,
    runs: Vec<RunJson>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ParaStyleJson {
    Normal,
    Heading {
        level: u8,
    },
    List {
        list_kind: String,
        level: u32,
        number_format: String,
    },
}

#[derive(Serialize)]
struct ParaPropsJson {
    align: Option<String>,
    spacing_before_pt: f64,
    spacing_after_pt: f64,
    line_spacing: LineSpacingJson,
    indent_left_pt: f64,
    indent_first_pt: f64,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum LineSpacingJson {
    Single,
    AtLeast { pt: f64 },
    Exactly { pt: f64 },
    Multiple { factor: f64 },
}

#[derive(Serialize)]
struct RunJson {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    size_pt: Option<f64>,
    color: Option<String>,
    font_family: Option<String>,
    highlight: bool,
    superscript: bool,
    subscript: bool,
    link: Option<String>,
}

#[derive(Serialize)]
struct TableJson {
    col_widths_pt: Vec<f64>,
    rows: Vec<TableRowJson>,
    borders: bool,
}

#[derive(Serialize)]
struct TableRowJson {
    cells: Vec<TableCellJson>,
}

#[derive(Serialize)]
struct TableCellJson {
    colspan: u32,
    rowspan: u32,
    shading: Option<String>,
    blocks: Vec<BlockJson>,
}

#[derive(Serialize)]
struct ImageJson {
    path: String,
    width_pt: f64,
    height_pt: f64,
}

fn color_hex(c: &RgbColor) -> String {
    c.hex()
}

fn align_name(a: Align) -> String {
    match a {
        Align::Left => "left".to_owned(),
        Align::Center => "center".to_owned(),
        Align::Right => "right".to_owned(),
        Align::Justify => "justify".to_owned(),
    }
}

fn line_spacing_json(ls: &LineSpacing) -> LineSpacingJson {
    match ls {
        LineSpacing::Single => LineSpacingJson::Single,
        LineSpacing::AtLeast(v) => LineSpacingJson::AtLeast { pt: *v },
        LineSpacing::Exactly(v) => LineSpacingJson::Exactly { pt: *v },
        LineSpacing::Multiple(v) => LineSpacingJson::Multiple { factor: *v },
    }
}

fn run_json(r: &Run) -> RunJson {
    RunJson {
        text: r.text.clone(),
        bold: r.bold,
        italic: r.italic,
        underline: r.underline,
        strike: r.strike,
        size_pt: r.size_pt,
        color: r.color.as_ref().map(color_hex),
        font_family: r.font_family.as_ref().map(|f| map_word_font(f).to_owned()),
        highlight: r.highlight,
        superscript: r.superscript,
        subscript: r.subscript,
        link: r.link.clone(),
    }
}

fn para_style_json(s: &ParagraphStyle) -> ParaStyleJson {
    match s {
        ParagraphStyle::Normal => ParaStyleJson::Normal,
        ParagraphStyle::Heading(level) => ParaStyleJson::Heading { level: *level },
        ParagraphStyle::List(ls) => ParaStyleJson::List {
            list_kind: match ls.kind {
                ListKind::Bullet => "bullet".to_owned(),
                ListKind::Ordered => "ordered".to_owned(),
            },
            level: ls.level,
            number_format: ls.number_format.clone(),
        },
    }
}

fn para_props_json(p: &ParagraphProps) -> ParaPropsJson {
    ParaPropsJson {
        align: p.align.map(align_name),
        spacing_before_pt: p.spacing_before_pt,
        spacing_after_pt: p.spacing_after_pt,
        line_spacing: line_spacing_json(&p.line_spacing),
        indent_left_pt: p.indent_left_pt,
        indent_first_pt: p.indent_first_pt,
    }
}

fn block_json(block: &Block, img_idx: &mut usize) -> BlockJson {
    match block {
        Block::Paragraph(p) => BlockJson::Paragraph(ParagraphJson {
            style: para_style_json(&p.style),
            props: para_props_json(&p.props),
            runs: p.runs.iter().map(run_json).collect(),
        }),
        Block::Table(t) => BlockJson::Table(TableJson {
            col_widths_pt: t.col_widths_pt.clone(),
            borders: t.borders,
            rows: t
                .rows
                .iter()
                .map(|row| TableRowJson {
                    cells: row
                        .cells
                        .iter()
                        .map(|c| TableCellJson {
                            colspan: c.colspan,
                            rowspan: c.rowspan,
                            shading: c.shading.as_ref().map(color_hex),
                            blocks: c.blocks.iter().map(|b| block_json(b, img_idx)).collect(),
                        })
                        .collect(),
                })
                .collect(),
        }),
        Block::Image(img) => {
            let path = format!("media/img{img_idx}.{}", ext_for_kind(&img.kind));
            *img_idx += 1;
            BlockJson::Image(ImageJson {
                path,
                width_pt: img.width_pt,
                height_pt: img.height_pt,
            })
        }
        Block::PageBreak => BlockJson::PageBreak,
    }
}

fn ext_for_kind(k: &ImageKind) -> &'static str {
    match k {
        ImageKind::Jpeg => "jpg",
        ImageKind::Png => "png",
    }
}

pub fn collect_image_files(doc: &Document) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    for block in &doc.blocks {
        if let Block::Image(img) = block {
            let name = format!("media/img{idx}.{}", ext_for_kind(&img.kind));
            out.push((name, img.bytes.clone()));
            idx += 1;
        }
    }
    out
}

pub fn to_json(doc: &Document) -> serde_json::Result<String> {
    let mut img_idx = 0usize;
    let blocks: Vec<BlockJson> = doc
        .blocks
        .iter()
        .map(|b| block_json(b, &mut img_idx))
        .collect();
    let orientation = match doc.page.orientation {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    };
    let header = doc.header.as_ref().map(|h| HeaderJson {
        runs: h.runs.iter().map(run_json).collect(),
    });
    let footer = doc.footer.as_ref().map(|h| HeaderJson {
        runs: h.runs.iter().map(run_json).collect(),
    });
    let json = DocJson {
        page: PageJson {
            width_pt: doc.page.width_pt,
            height_pt: doc.page.height_pt,
            margin_top_pt: doc.page.margin_top_pt,
            margin_right_pt: doc.page.margin_right_pt,
            margin_bottom_pt: doc.page.margin_bottom_pt,
            margin_left_pt: doc.page.margin_left_pt,
            orientation: orientation.to_owned(),
        },
        header,
        footer,
        blocks,
    };
    serde_json::to_string(&json)
}

pub fn map_word_font(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.contains("calibri") {
        "Carlito"
    } else if lower.contains("cambria") {
        "Caladea"
    } else if lower.contains("arial") {
        "Liberation Sans"
    } else if lower.contains("times") {
        "Liberation Serif"
    } else if lower.contains("courier") {
        "Liberation Mono"
    } else {
        "DejaVu Sans"
    }
}

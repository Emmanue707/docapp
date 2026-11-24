use crate::types::{ImageKind, RgbColor};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageSetup {
    pub width_pt: f64,
    pub height_pt: f64,
    pub margin_top_pt: f64,
    pub margin_right_pt: f64,
    pub margin_bottom_pt: f64,
    pub margin_left_pt: f64,
    pub orientation: Orientation,
}

impl Default for PageSetup {
    fn default() -> Self {
        Self {
            width_pt: 595.28,
            height_pt: 841.89,
            margin_top_pt: 72.0,
            margin_right_pt: 72.0,
            margin_bottom_pt: 72.0,
            margin_left_pt: 72.0,
            orientation: Orientation::Portrait,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ParagraphProps {
    pub align: Option<Align>,
    pub spacing_before_pt: f64,
    pub spacing_after_pt: f64,
    pub line_spacing: LineSpacing,
    pub indent_left_pt: f64,
    pub indent_first_pt: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum LineSpacing {
    #[default]
    Single,
    AtLeast(f64),
    Exactly(f64),
    Multiple(f64),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ListKind {
    #[default]
    Bullet,
    Ordered,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListStyle {
    pub kind: ListKind,
    pub level: u32,
    pub number_format: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParagraphStyle {
    Normal,
    Heading(u8),
    List(ListStyle),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Paragraph {
    pub style: ParagraphStyle,
    pub props: ParagraphProps,
    pub runs: Vec<Run>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub size_pt: Option<f64>,
    pub color: Option<RgbColor>,
    pub font_family: Option<String>,
    pub highlight: bool,
    pub superscript: bool,
    pub subscript: bool,
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableCell {
    pub blocks: Vec<Block>,
    pub colspan: u32,
    pub rowspan: u32,
    pub shading: Option<RgbColor>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub col_widths_pt: Vec<f64>,
    pub rows: Vec<TableRow>,
    pub borders: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageBlock {
    pub bytes: Vec<u8>,
    pub kind: ImageKind,
    pub width_pt: f64,
    pub height_pt: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
    Image(ImageBlock),
    PageBreak,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeaderFooter {
    pub runs: Vec<Run>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Document {
    pub page: PageSetup,
    pub blocks: Vec<Block>,
    pub header: Option<HeaderFooter>,
    pub footer: Option<HeaderFooter>,
}

impl Document {
    pub fn paragraph(mut self, text: &str) -> Self {
        self.blocks.push(Block::Paragraph(Paragraph {
            style: ParagraphStyle::Normal,
            props: ParagraphProps::default(),
            runs: vec![Run {
                text: text.to_owned(),
                ..Run::default()
            }],
        }));
        self
    }

    pub fn heading(mut self, level: u8, text: &str) -> Self {
        self.blocks.push(Block::Paragraph(Paragraph {
            style: ParagraphStyle::Heading(level),
            props: ParagraphProps::default(),
            runs: vec![Run {
                text: text.to_owned(),
                bold: true,
                ..Run::default()
            }],
        }));
        self
    }

    pub fn plain_text(&self) -> String {
        document_plain_text(self)
    }
}

pub fn document_plain_text(doc: &Document) -> String {
    let mut out = String::new();
    for block in &doc.blocks {
        match block {
            Block::Paragraph(p) => {
                for (i, run) in p.runs.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push_str(&run.text);
                }
                out.push('\n');
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for (i, cell) in row.cells.iter().enumerate() {
                        if i > 0 {
                            out.push('\t');
                        }
                        for b in &cell.blocks {
                            if let Block::Paragraph(p) = b {
                                for run in &p.runs {
                                    out.push_str(&run.text);
                                }
                            }
                        }
                    }
                    out.push('\n');
                }
            }
            Block::Image(_) => {}
            Block::PageBreak => out.push('\x0c'),
        }
    }
    out
}

pub fn runs_equivalent(a: &[Run], b: &[Run]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).all(|(x, y)| {
        x.text == y.text && x.bold == y.bold && x.italic == y.italic && x.underline == y.underline
    })
}

pub fn documents_text_equivalent(a: &Document, b: &Document) -> bool {
    let pa = normalize_plain(&document_plain_text(a));
    let pb = normalize_plain(&document_plain_text(b));
    pa == pb
}

fn normalize_plain(s: &str) -> String {
    s.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(feature = "typst")]
pub mod typst_json;

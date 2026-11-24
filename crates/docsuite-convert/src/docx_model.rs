use std::collections::HashMap;

use docsuite_core::Result;

use crate::doc::{
    Align, Block, Document, HeaderFooter, ImageBlock, LineSpacing, ListKind, ListStyle, Paragraph,
    ParagraphProps, ParagraphStyle, Run, Table, TableCell, TableRow,
};
use crate::docx::escape_xml;
use crate::ooxf::OoxWriter;
use crate::types::{ImageKind, RgbColor};

const TWIPS_PER_PT: i64 = 20;
const EMU_PER_PT: i64 = 12700;

pub fn write_docx_from_document(path: &std::path::Path, doc: &Document) -> Result<()> {
    let mut images: Vec<(String, Vec<u8>, ImageKind)> = Vec::new();
    let mut img_i = 0usize;
    collect_images(doc, &mut images, &mut img_i);
    let mut zip = OoxWriter::create(path)?;
    write_parts(&mut zip, doc, &images)?;
    zip.finish()
}

fn collect_images(doc: &Document, out: &mut Vec<(String, Vec<u8>, ImageKind)>, idx: &mut usize) {
    for block in &doc.blocks {
        collect_block_images(block, out, idx);
    }
}

fn collect_block_images(
    block: &Block,
    out: &mut Vec<(String, Vec<u8>, ImageKind)>,
    idx: &mut usize,
) {
    match block {
        Block::Image(img) => {
            let ext = match img.kind {
                ImageKind::Jpeg => "jpeg",
                ImageKind::Png => "png",
            };
            let name = format!("word/media/image{idx}.{ext}");
            out.push((name, img.bytes.clone(), img.kind));
            *idx += 1;
        }
        Block::Table(t) => {
            for row in &t.rows {
                for cell in &row.cells {
                    for b in &cell.blocks {
                        collect_block_images(b, out, idx);
                    }
                }
            }
        }
        _ => {}
    }
}

fn write_parts(
    zip: &mut OoxWriter,
    doc: &Document,
    images: &[(String, Vec<u8>, ImageKind)],
) -> Result<()> {
    zip.write_str("[Content_Types].xml", &content_types(images))?;
    zip.write_str("_rels/.rels", ROOT_RELS)?;
    zip.write_str("word/document.xml", &document_xml(doc, images.len()))?;
    zip.write_str("word/_rels/document.xml.rels", &document_rels(images))?;
    zip.write_str("word/styles.xml", STYLES_XML)?;
    zip.write_str("word/numbering.xml", NUMBERING_XML)?;
    zip.write_str("word/settings.xml", SETTINGS_XML)?;
    zip.write_str("word/fontTable.xml", FONT_TABLE_XML)?;
    zip.write_str("docProps/core.xml", CORE_XML)?;
    zip.write_str("docProps/app.xml", APP_XML)?;
    for (name, data, _) in images {
        zip.write(name, data)?;
    }
    Ok(())
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal">
<w:name w:val="Normal"/>
<w:qFormat/>
</w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:b/><w:sz w:val="32"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:b/><w:sz w:val="28"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading3"><w:name w:val="heading 3"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="2"/></w:pPr><w:rPr><w:b/><w:sz w:val="26"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading4"><w:name w:val="heading 4"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="3"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading5"><w:name w:val="heading 5"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="4"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading6"><w:name w:val="heading 6"/><w:basedOn w:val="Normal"/><w:pPr><w:outlineLvl w:val="5"/></w:pPr><w:rPr><w:b/></w:rPr></w:style>
</w:styles>"#;

const NUMBERING_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:lvlJc w:val="left"/></w:lvl></w:abstractNum>
<w:abstractNum w:abstractNumId="1"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:lvlJc w:val="left"/></w:lvl></w:abstractNum>
<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>"#;

const SETTINGS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:settings>"#;

const FONT_TABLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:fonts>"#;

const CORE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
<dc:title>docapp</dc:title>
</cp:coreProperties>"#;

const APP_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
<Application>docapp</Application>
</Properties>"#;

fn content_types(images: &[(String, Vec<u8>, ImageKind)]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>"#,
    );
    let mut seen = HashMap::new();
    for (_, _, kind) in images {
        let ext = match kind {
            ImageKind::Jpeg => "jpeg",
            ImageKind::Png => "png",
        };
        if seen.insert(ext, ()).is_none() {
            let ct = match kind {
                ImageKind::Jpeg => "image/jpeg",
                ImageKind::Png => "image/png",
            };
            xml.push_str(&format!(
                r#"<Default Extension="{ext}" ContentType="{ct}"/>"#
            ));
        }
    }
    xml.push_str(r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>
<Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
</Types>"#);
    xml
}

fn document_rels(images: &[(String, Vec<u8>, ImageKind)]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
"#,
    );
    for (i, (name, _, _)) in images.iter().enumerate() {
        let target = name.strip_prefix("word/").unwrap_or(name);
        xml.push_str(&format!(
            r#"<Relationship Id="rIdImg{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{target}"/>"#
        ));
    }
    xml.push_str("</Relationships>");
    xml
}

fn document_xml(doc: &Document, _img_count: usize) -> String {
    let w = (doc.page.width_pt * TWIPS_PER_PT as f64) as i64;
    let h = (doc.page.height_pt * TWIPS_PER_PT as f64) as i64;
    let m_t = (doc.page.margin_top_pt * TWIPS_PER_PT as f64) as i64;
    let m_r = (doc.page.margin_right_pt * TWIPS_PER_PT as f64) as i64;
    let m_b = (doc.page.margin_bottom_pt * TWIPS_PER_PT as f64) as i64;
    let m_l = (doc.page.margin_left_pt * TWIPS_PER_PT as f64) as i64;
    let mut body = String::new();
    let mut img_i = 0usize;
    for block in &doc.blocks {
        body.push_str(&block_xml(block, &mut img_i));
    }
    let sect = format!(
        r#"<w:sectPr><w:pgSz w:w="{w}" w:h="{h}"/><w:pgMar w:top="{m_t}" w:right="{m_r}" w:bottom="{m_b}" w:left="{m_l}"/></w:sectPr>"#
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
<w:body>{body}{sect}</w:body>
</w:document>"#
    )
}

fn block_xml(block: &Block, img_i: &mut usize) -> String {
    match block {
        Block::Paragraph(p) => paragraph_xml(p),
        Block::Table(t) => table_xml(t, img_i),
        Block::Image(img) => {
            let s = image_inline_xml(img, *img_i);
            *img_i += 1;
            s
        }
        Block::PageBreak => r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#.to_owned(),
    }
}

fn paragraph_xml(p: &Paragraph) -> String {
    let mut ppr = String::new();
    match &p.style {
        ParagraphStyle::Heading(level) => {
            ppr.push_str(&format!(r#"<w:pStyle w:val="Heading{level}"/>"#));
        }
        ParagraphStyle::List(ls) => {
            let num = match ls.kind {
                ListKind::Ordered => 1,
                ListKind::Bullet => 2,
            };
            ppr.push_str(&format!(
                r#"<w:numPr><w:ilvl w:val="{}"/><w:numId w:val="{num}"/></w:numPr>"#,
                ls.level
            ));
        }
        ParagraphStyle::Normal => {}
    }
    if let Some(align) = p.props.align {
        ppr.push_str(&format!(r#"<w:jc w:val="{}"/>"#, align_val(align)));
    }
    let ppr_wrap = if ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{ppr}</w:pPr>")
    };
    let runs: String = p.runs.iter().map(run_xml).collect();
    format!("<w:p>{ppr_wrap}{runs}</w:p>")
}

fn align_val(a: Align) -> &'static str {
    match a {
        Align::Left => "left",
        Align::Center => "center",
        Align::Right => "right",
        Align::Justify => "both",
    }
}

fn run_xml(r: &Run) -> String {
    let mut rpr = String::new();
    if r.bold {
        rpr.push_str("<w:b/>");
    }
    if r.italic {
        rpr.push_str("<w:i/>");
    }
    if r.underline {
        rpr.push_str(r#"<w:u w:val="single"/>"#);
    }
    if r.strike {
        rpr.push_str("<w:strike/>");
    }
    if let Some(sz) = r.size_pt {
        let half = (sz * 2.0).round() as i64;
        rpr.push_str(&format!(r#"<w:sz w:val="{half}"/>"#));
    }
    if let Some(c) = &r.color {
        rpr.push_str(&format!(r#"<w:color w:val="{}"/>"#, c.hex()));
    }
    if let Some(f) = &r.font_family {
        let f = escape_xml(f);
        rpr.push_str(&format!(r#"<w:rFonts w:ascii="{f}" w:hAnsi="{f}"/>"#));
    }
    if r.highlight {
        rpr.push_str(r#"<w:highlight w:val="yellow"/>"#);
    }
    let rpr_wrap = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    let text = escape_xml(&r.text);
    let preserve = if r.text.starts_with(' ') || r.text.ends_with(' ') {
        r#" xml:space="preserve""#
    } else {
        ""
    };
    format!("<w:r>{rpr_wrap}<w:t{preserve}>{text}</w:t></w:r>")
}

fn table_xml(t: &Table, img_i: &mut usize) -> String {
    let mut out = String::from(
        r#"<w:tbl><w:tblPr><w:tblBorders><w:top w:val="single" w:sz="4"/><w:left w:val="single" w:sz="4"/><w:bottom w:val="single" w:sz="4"/><w:right w:val="single" w:sz="4"/><w:insideH w:val="single" w:sz="4"/><w:insideV w:val="single" w:sz="4"/></w:tblBorders></w:tblPr>"#,
    );
    for row in &t.rows {
        out.push_str("<w:tr>");
        for cell in &row.cells {
            let mut inner = String::new();
            for b in &cell.blocks {
                inner.push_str(&block_xml(b, img_i));
            }
            if inner.is_empty() {
                inner.push_str("<w:p/>");
            }
            let span = if cell.colspan > 1 {
                format!(r#"<w:gridSpan w:val="{}"/>"#, cell.colspan)
            } else {
                String::new()
            };
            out.push_str(&format!("<w:tc><w:tcPr>{span}</w:tcPr>{inner}</w:tc>"));
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    out
}

fn image_inline_xml(img: &ImageBlock, idx: usize) -> String {
    let w = (img.width_pt * EMU_PER_PT as f64) as i64;
    let h = (img.height_pt * EMU_PER_PT as f64) as i64;
    let rid = format!("rIdImg{idx}");
    format!(
        r#"<w:p><w:r><w:drawing>
<wp:inline distT="0" distB="0" distL="0" distR="0">
<wp:extent cx="{w}" cy="{h}"/>
<wp:docPr id="{idx}" name="Image{idx}"/>
<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
<pic:pic><pic:nvPicPr><pic:cNvPr id="{idx}" name="img{idx}"/><pic:cNvPicPr/></pic:nvPicPr>
<pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>
<pic:spPr><a:xfrm><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"/></pic:spPr>
</pic:pic></a:graphicData></a:graphic>
</wp:inline></w:drawing></w:r></w:p>"#
    )
}

pub fn read_docx_document(path: &std::path::Path) -> Result<Document> {
    let pkg = std::fs::read(path).map_err(docsuite_core::Error::from)?;
    crate::docx_read::read_docx_package(&pkg)
}

pub fn image_block_document(
    bytes: Vec<u8>,
    kind: ImageKind,
    page: &crate::doc::PageSetup,
) -> Document {
    let (w, h) = image_fit_pt(bytes.as_slice(), kind, page);
    Document {
        page: page.clone(),
        blocks: vec![Block::Image(ImageBlock {
            bytes,
            kind,
            width_pt: w,
            height_pt: h,
        })],
        header: None,
        footer: None,
    }
}

fn image_fit_pt(bytes: &[u8], _kind: ImageKind, page: &crate::doc::PageSetup) -> (f64, f64) {
    let (iw, ih) = image_dims(bytes, _kind);
    let max_w = page.width_pt - page.margin_left_pt - page.margin_right_pt;
    let max_h = page.height_pt - page.margin_top_pt - page.margin_bottom_pt;
    if iw <= 0.0 || ih <= 0.0 {
        return (max_w, max_h);
    }
    let scale = (max_w / iw).min(max_h / ih).min(1.0);
    (iw * scale, ih * scale)
}

fn image_dims(bytes: &[u8], _kind: ImageKind) -> (f64, f64) {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes));
    if let Ok(reader) = reader.with_guessed_format()
        && let Ok(img) = reader.decode()
    {
        let w = img.width() as f64 * 72.0 / 96.0;
        let h = img.height() as f64 * 72.0 / 96.0;
        return (w, h);
    }
    (200.0, 200.0)
}

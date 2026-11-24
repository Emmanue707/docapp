use docsuite_core::Result;

use crate::doc::{
    Align, Block, Document, ImageBlock, ListKind, Paragraph, ParagraphStyle, Run, Table, TableCell,
    TableRow,
};
use crate::docx::escape_xml;
use crate::ooxf::OdfWriter;
use crate::types::ImageKind;

pub fn write_odt_from_document(path: &std::path::Path, doc: &Document) -> Result<()> {
    let mut images: Vec<(String, Vec<u8>, ImageKind)> = Vec::new();
    let mut idx = 0usize;
    collect_images(doc, &mut images, &mut idx);
    let content = content_xml(doc, &images);
    let styles = styles_xml();
    let meta = meta_xml();
    let manifest = manifest_xml(&images);
    let mut w = OdfWriter::new();
    w.write_stored("mimetype", b"application/vnd.oasis.opendocument.text");
    w.write_deflated("content.xml", content.as_bytes());
    w.write_deflated("styles.xml", styles.as_bytes());
    w.write_deflated("meta.xml", meta.as_bytes());
    w.write_deflated("META-INF/manifest.xml", manifest.as_bytes());
    for (name, data, _) in &images {
        w.write_deflated(name, data);
    }
    w.finish(path)
}

fn collect_images(doc: &Document, out: &mut Vec<(String, Vec<u8>, ImageKind)>, idx: &mut usize) {
    for block in &doc.blocks {
        collect_block(block, out, idx);
    }
}

fn collect_block(block: &Block, out: &mut Vec<(String, Vec<u8>, ImageKind)>, idx: &mut usize) {
    match block {
        Block::Image(img) => {
            let ext = match img.kind {
                ImageKind::Jpeg => "jpg",
                ImageKind::Png => "png",
            };
            out.push((
                format!("Pictures/img{idx}.{ext}"),
                img.bytes.clone(),
                img.kind,
            ));
            *idx += 1;
        }
        Block::Table(t) => {
            for row in &t.rows {
                for cell in &row.cells {
                    for b in &cell.blocks {
                        collect_block(b, out, idx);
                    }
                }
            }
        }
        _ => {}
    }
}

fn content_xml(doc: &Document, images: &[(String, Vec<u8>, ImageKind)]) -> String {
    let mut body = String::new();
    let mut img_i = 0usize;
    for block in &doc.blocks {
        body.push_str(&block_xml(block, images, &mut img_i));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"
xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
xmlns:xlink="http://www.w3.org/1999/xlink"
xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
office:version="1.3">
<office:body><office:text>{body}</office:text></office:body>
</office:document-content>"#
    )
}

fn block_xml(block: &Block, images: &[(String, Vec<u8>, ImageKind)], img_i: &mut usize) -> String {
    match block {
        Block::Paragraph(p) => paragraph_xml(p),
        Block::Table(t) => table_xml(t, images, img_i),
        Block::Image(img) => image_xml(img, images, img_i),
        Block::PageBreak => r#"<text:p><text:soft-page-break/></text:p>"#.to_owned(),
    }
}

fn paragraph_xml(p: &Paragraph) -> String {
    let tag = match &p.style {
        ParagraphStyle::Heading(level) => {
            return format!(
                r#"<text:h text:outline-level="{level}">{}</text:h>"#,
                runs_xml(&p.runs)
            );
        }
        ParagraphStyle::List(ls) => {
            let bullet = match ls.kind {
                ListKind::Bullet => "• ",
                ListKind::Ordered => "",
            };
            let mut runs = p.runs.clone();
            if !runs.is_empty() && !bullet.is_empty() {
                runs[0].text = format!("{bullet}{}", runs[0].text);
            }
            return format!(
                r#"<text:list><text:list-item><text:p>{}</text:p></text:list-item></text:list>"#,
                runs_xml(&runs)
            );
        }
        ParagraphStyle::Normal => "text:p",
    };
    let _ = tag;
    format!("<text:p>{}</text:p>", runs_xml(&p.runs))
}

fn runs_xml(runs: &[Run]) -> String {
    runs.iter()
        .map(|r| {
            let mut attrs = String::new();
            if r.bold {
                attrs.push_str(r#" text:style-name="Bold""#);
            }
            if r.italic {
                attrs.push_str(r#" text:style-name="Italic""#);
            }
            let text = escape_xml(&r.text);
            if let Some(h) = &r.link {
                format!(
                    r#"<text:a xlink:href="{}" xlink:type="simple"><text:span{attrs}>{text}</text:span></text:a>"#,
                    escape_xml(h)
                )
            } else if attrs.is_empty() {
                format!("<text:span>{text}</text:span>")
            } else {
                format!(r#"<text:span{attrs}>{text}</text:span>"#)
            }
        })
        .collect()
}

fn table_xml(t: &Table, images: &[(String, Vec<u8>, ImageKind)], img_i: &mut usize) -> String {
    let mut out = String::from(r#"<table:table table:style-name="Table1">"#);
    for row in &t.rows {
        out.push_str("<table:table-row>");
        for cell in &row.cells {
            out.push_str(r#"<table:table-cell table:style-name="TableCell">"#);
            for b in &cell.blocks {
                out.push_str(&block_xml(b, images, img_i));
            }
            if cell.blocks.is_empty() {
                out.push_str("<text:p/>");
            }
            out.push_str("</table:table-cell>");
        }
        out.push_str("</table:table-row>");
    }
    out.push_str("</table:table>");
    out
}

fn image_xml(
    img: &ImageBlock,
    images: &[(String, Vec<u8>, ImageKind)],
    img_i: &mut usize,
) -> String {
    let (path, _, _) = &images[*img_i];
    *img_i += 1;
    let w = format!("{:.2}cm", img.width_pt / 72.0 * 2.54);
    let h = format!("{:.2}cm", img.height_pt / 72.0 * 2.54);
    format!(
        r#"<text:p><draw:frame draw:style-name="fr1" svg:width="{w}" svg:height="{h}">
<draw:image xlink:href="{path}" xlink:type="simple"/>
</draw:frame></text:p>"#
    )
}

fn styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-styles xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
xmlns:style="urn:oasis:names:tc:opendocument:xmlns:style:1.0"
xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"
xmlns:fo="urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0"
office:version="1.3">
<office:styles>
<style:style style:name="Bold" style:family="text"><style:text-properties fo:font-weight="bold"/></style:style>
<style:style style:name="Italic" style:family="text"><style:text-properties fo:font-style="italic"/></style:style>
<style:style style:name="Heading1" style:family="paragraph" style:parent-style-name="Standard">
<style:text-properties fo:font-size="18pt" fo:font-weight="bold"/>
</style:style>
</office:styles>
</office:document-styles>"#
        .to_owned()
}

fn meta_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8"?>
<office:document-meta xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"
xmlns:meta="urn:oasis:names:tc:opendocument:xmlns:meta:1.0"
office:version="1.3">
<office:meta><meta:generator>docapp</meta:generator></office:meta>
</office:document-meta>"#
        .to_owned()
}

fn manifest_xml(images: &[(String, Vec<u8>, ImageKind)]) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0">
<manifest:file-entry manifest:full-path="/" manifest:media-type="application/vnd.oasis.opendocument.text"/>
<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="styles.xml" manifest:media-type="text/xml"/>
<manifest:file-entry manifest:full-path="meta.xml" manifest:media-type="text/xml"/>"#,
    );
    for (path, _, _) in images {
        xml.push_str(&format!(
            r#"<manifest:file-entry manifest:full-path="{path}" manifest:media-type="image/png"/>"#
        ));
    }
    xml.push_str("</manifest:manifest>");
    xml
}

pub fn read_odt_document(path: &std::path::Path) -> Result<Document> {
    let pkg = std::fs::read(path).map_err(docsuite_core::Error::from)?;
    crate::odt_read::read_odt_package(&pkg)
}

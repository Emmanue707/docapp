use std::io::Write;

#[cfg(test)]
use std::io::Cursor;

use docsuite_core::Result;
use zip::ZipWriter;
use zip::result::ZipError;
use zip::write::SimpleFileOptions;

use crate::PdfToWordMode;
use crate::obfuscate::{font_guid_key, obfuscate_font};
use crate::types::{ExtractedPage, FontEmbed, ImageKind, PlacedImage, TextRun, VectorPath};

const EMU_PER_PT: i64 = 12700;
const TWIPS_PER_PT: i64 = 20;

pub fn write_docx(
    path: &std::path::Path,
    pages: &[ExtractedPage],
    mode: PdfToWordMode,
    embed_fonts: bool,
    fonts: &[FontEmbed],
) -> Result<()> {
    let file = std::fs::File::create(path).map_err(docsuite_core::Error::from)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    write_content_types(&mut zip, opts, fonts, pages)?;
    write_root_rels(&mut zip, opts)?;
    write_document(&mut zip, opts, pages, mode, embed_fonts, fonts)?;
    write_document_rels(&mut zip, opts, fonts, pages)?;
    write_styles(&mut zip, opts)?;
    write_settings(&mut zip, opts, embed_fonts)?;
    write_font_table(&mut zip, opts, embed_fonts, fonts)?;
    write_core(&mut zip, opts)?;
    write_app(&mut zip, opts)?;
    for (i, page) in pages.iter().enumerate() {
        for (j, img) in page.images.iter().enumerate() {
            let name = format!("word/media/image_p{i}_{j}.{}", ext_for(&img.kind));
            zip.start_file(name, opts).map_err(zip_err)?;
            zip.write_all(&img.bytes)
                .map_err(docsuite_core::Error::from)?;
        }
    }
    if embed_fonts {
        for (i, font) in fonts.iter().enumerate() {
            let mut data = font.ttf_bytes.clone();
            let guid = font_guid_key(i);
            obfuscate_font(&mut data, &guid);
            let name = format!("word/fonts/font{i}.odttf");
            zip.start_file(name, opts).map_err(zip_err)?;
            zip.write_all(&data).map_err(docsuite_core::Error::from)?;
        }
    }
    zip.finish().map_err(zip_err)?;
    Ok(())
}

fn ext_for(kind: &ImageKind) -> &'static str {
    match kind {
        ImageKind::Jpeg => "jpeg",
        ImageKind::Png => "png",
    }
}

fn zip_err(e: ZipError) -> docsuite_core::Error {
    docsuite_core::Error::Convert(e.to_string())
}

fn write_content_types(
    zip: &mut ZipWriter<std::fs::File>,
    opts: SimpleFileOptions,
    fonts: &[FontEmbed],
    pages: &[ExtractedPage],
) -> Result<()> {
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">",
    );
    xml.push_str(r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#);
    xml.push_str(r#"<Default Extension="xml" ContentType="application/xml"/>"#);
    for p in pages {
        for img in &p.images {
            let ct = match img.kind {
                ImageKind::Jpeg => "image/jpeg",
                ImageKind::Png => "image/png",
            };
            xml.push_str(&format!(
                r#"<Default Extension="{}" ContentType="{}"/>"#,
                ext_for(&img.kind),
                ct
            ));
        }
    }
    if !fonts.is_empty() {
        xml.push_str(r#"<Default Extension="odttf" ContentType="application/vnd.openxmlformats-officedocument.obfuscatedFont"/>"#);
    }
    xml.push_str(r#"<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>"#);
    xml.push_str(r#"<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#);
    xml.push_str(r#"<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>"#);
    xml.push_str(r#"<Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>"#);
    xml.push_str(r#"<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>"#);
    xml.push_str(r#"<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#);
    xml.push_str("</Types>");
    zip.start_file("[Content_Types].xml", opts)
        .map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_root_rels(zip: &mut ZipWriter<std::fs::File>, opts: SimpleFileOptions) -> Result<()> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;
    zip.start_file("_rels/.rels", opts).map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_document(
    zip: &mut ZipWriter<std::fs::File>,
    opts: SimpleFileOptions,
    pages: &[ExtractedPage],
    mode: PdfToWordMode,
    embed_fonts: bool,
    fonts: &[FontEmbed],
) -> Result<()> {
    let mut body = String::new();
    match mode {
        PdfToWordMode::ExactLook => {
            for (pi, page) in pages.iter().enumerate() {
                body.push_str(&exact_page_body(page, pi));
            }
        }
        PdfToWordMode::Editable => {
            for (pi, page) in pages.iter().enumerate() {
                body.push_str(&editable_page_body(page, pi));
                if pi + 1 < pages.len() {
                    body.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
                }
            }
        }
    }
    let _ = embed_fonts;
    let _ = fonts;
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
 xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
 xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
 xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"
 xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
 xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
 mc:Ignorable="wps">
<w:body>{body}</w:body>
</w:document>"#
    );
    zip.start_file("word/document.xml", opts).map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn exact_page_body(page: &ExtractedPage, page_index: usize) -> String {
    let mut out = String::new();
    let w_twips = (page.width * TWIPS_PER_PT as f64) as i64;
    let h_twips = (page.height * TWIPS_PER_PT as f64) as i64;
    let pg_sz = if page.width > page.height {
        format!(r#"<w:pgSz w:w="{w_twips}" w:h="{h_twips}" w:orient="landscape"/>"#)
    } else {
        format!(r#"<w:pgSz w:w="{w_twips}" w:h="{h_twips}"/>"#)
    };
    let mut z: u32 = 0;
    for vp in &page.vectors {
        out.push_str(&vector_anchor(vp, page_index, z));
        z += 1;
    }
    for (ii, img) in page.images.iter().enumerate() {
        out.push_str(&image_anchor(img, page_index, ii, z + 100));
        z += 1;
    }
    for (ti, run) in page.text.iter().enumerate() {
        out.push_str(&text_anchor(run, page_index, ti, z + 1000));
    }
    out.push_str(&format!(
        r#"<w:sectPr>{pg_sz}<w:pgMar w:top="0" w:right="0" w:bottom="0" w:left="0" w:header="0" w:footer="0" w:gutter="0"/></w:sectPr>"#
    ));
    out
}

fn pt_to_emu(v: f64) -> i64 {
    (v * EMU_PER_PT as f64).round() as i64
}

fn text_anchor(run: &TextRun, _pi: usize, ti: usize, z: u32) -> String {
    let x = pt_to_emu(run.x);
    let y = pt_to_emu(run.y);
    let w = pt_to_emu(run.width + run.font_size * 0.2);
    let h = pt_to_emu(run.font_size * 1.2);
    let sz = (run.font_size * 2.0).round() as i64;
    let color = run.color.hex();
    let b = if run.bold { r#"<w:b/>"# } else { "" };
    let i = if run.italic { r#"<w:i/>"# } else { "" };
    let family = escape_xml(&run.font_key);
    let text = escape_xml(&run.text);
    format!(
        r#"<w:p><w:r><w:alternateContent><w:choice Requires="wps"><w:drawing>
<wp:anchor behindDoc="0" distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="{z}" locked="0" layoutInCell="0" allowOverlap="1">
<wp:simplePos x="0" y="0"/>
<wp:positionH relativeFrom="page"><wp:posOffset>{x}</wp:posOffset></wp:positionH>
<wp:positionV relativeFrom="page"><wp:posOffset>{y}</wp:posOffset></wp:positionV>
<wp:extent cx="{w}" cy="{h}"/>
<wp:wrapNone/>
<wp:docPr id="{ti}" name="Text{ti}"/>
<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
<wps:wsp><wps:cNvSpPr/><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"/></wps:spPr>
<wps:txbx><w:txbxContent><w:p><w:pPr><w:spacing w:line="{sz}" w:lineRule="exact"/></w:pPr>
<w:r><w:rPr><w:rFonts w:ascii="{family}" w:hAnsi="{family}"/><w:sz w:val="{sz}"/><w:color w:val="{color}"/>{b}{i}</w:rPr><w:t xml:space="preserve">{text}</w:t></w:r>
</w:p></w:txbxContent></wps:txbx><wps:bodyPr anchor="t" lIns="0" tIns="0" rIns="0" bIns="0"/></wps:wsp>
</a:graphicData></a:graphic>
</wp:anchor></w:drawing></w:choice></w:alternateContent></w:r></w:p>"#
    )
}

fn image_anchor(img: &PlacedImage, pi: usize, ii: usize, z: u32) -> String {
    let x = pt_to_emu(img.x);
    let y = pt_to_emu(img.y);
    let w = pt_to_emu(img.width);
    let h = pt_to_emu(img.height);
    let rid = format!("rIdImg{pi}_{ii}");
    format!(
        r#"<w:p><w:r><w:drawing>
<wp:anchor behindDoc="0" distT="0" distB="0" distL="0" distR="0" simplePos="0" relativeHeight="{z}" locked="0" layoutInCell="0" allowOverlap="1">
<wp:simplePos x="0" y="0"/>
<wp:positionH relativeFrom="page"><wp:posOffset>{x}</wp:posOffset></wp:positionH>
<wp:positionV relativeFrom="page"><wp:posOffset>{y}</wp:posOffset></wp:positionV>
<wp:extent cx="{w}" cy="{h}"/>
<wp:wrapNone/>
<wp:docPr id="{ii}" name="Image{ii}"/>
<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
<pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="img"/><pic:cNvPicPr/></pic:nvPicPr>
<pic:blipFill><a:blip r:embed="{rid}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"/>
<a:stretch><a:fillRect/></a:stretch></pic:blipFill>
<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{w}" cy="{h}"/></a:xfrm><a:prstGeom prst="rect"/></pic:spPr>
</pic:pic></a:graphicData></a:graphic>
</wp:anchor></w:drawing></w:r></w:p>"#
    )
}

fn vector_anchor(vp: &VectorPath, _idx: usize, z: u32) -> String {
    let (x, y, w, h) = path_bounds(vp);
    let x_emu = pt_to_emu(x);
    let y_emu = pt_to_emu(y);
    let w_emu = pt_to_emu(w.max(1.0));
    let h_emu = pt_to_emu(h.max(1.0));
    let mut geom = String::from("<a:pathLst><a:path w=\"100000\" h=\"100000\">");
    for seg in &vp.segments {
        match seg {
            crate::types::PathSegment::MoveTo(p) => {
                geom.push_str(&format!(
                    r#"<a:moveTo><a:pt x="{}" y="{}"/></a:moveTo>"#,
                    coord_pct(p.x, x, w),
                    coord_pct(p.y, y, h)
                ));
            }
            crate::types::PathSegment::LineTo(p) => {
                geom.push_str(&format!(
                    r#"<a:lnTo><a:pt x="{}" y="{}"/></a:lnTo>"#,
                    coord_pct(p.x, x, w),
                    coord_pct(p.y, y, h)
                ));
            }
            crate::types::PathSegment::Close => {
                geom.push_str("<a:close/>");
            }
            crate::types::PathSegment::CurveTo(_, _, _) => {}
        }
    }
    geom.push_str("</a:path></a:pathLst>");
    let fill = vp
        .fill
        .map(|c| {
            format!(
                r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
                c.hex()
            )
        })
        .unwrap_or_default();
    let stroke = vp
        .stroke
        .map(|c| {
            format!(
                r#"<a:ln w="12700"><a:solidFill><a:srgbClr val="{}"/></a:solidFill></a:ln>"#,
                c.hex()
            )
        })
        .unwrap_or_default();
    format!(
        r#"<w:p><w:r><w:drawing><wp:anchor behindDoc="1" relativeHeight="{z}" layoutInCell="0" allowOverlap="1">
<wp:positionH relativeFrom="page"><wp:posOffset>{x_emu}</wp:posOffset></wp:positionH>
<wp:positionV relativeFrom="page"><wp:posOffset>{y_emu}</wp:posOffset></wp:positionV>
<wp:extent cx="{w_emu}" cy="{h_emu}"/><wp:wrapNone/>
<a:graphic><a:graphicData uri="http://schemas.microsoft.com/office/word/2010/wordprocessingShape">
<wps:wsp><wps:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{w_emu}" cy="{h_emu}"/></a:xfrm>
<a:custGeom>{geom}</a:custGeom>{fill}{stroke}</wps:spPr></wps:wsp>
</a:graphicData></a:graphic></wp:anchor></w:drawing></w:r></w:p>"#
    )
}

fn coord_pct(v: f64, origin: f64, span: f64) -> i64 {
    if span.abs() < 0.001 {
        return 0;
    }
    (((v - origin) / span) * 100_000.0).round() as i64
}

fn path_bounds(vp: &VectorPath) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for seg in &vp.segments {
        let pts = match seg {
            crate::types::PathSegment::MoveTo(p) | crate::types::PathSegment::LineTo(p) => vec![p],
            crate::types::PathSegment::CurveTo(a, b, c) => vec![a, b, c],
            crate::types::PathSegment::Close => continue,
        };
        for p in pts {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
    }
    if min_x == f64::MAX {
        return (0.0, 0.0, 1.0, 1.0);
    }
    (min_x, min_y, max_x - min_x, max_y - min_y)
}

fn editable_page_body(page: &ExtractedPage, _pi: usize) -> String {
    let mut lines: Vec<&TextRun> = page.text.iter().collect();
    lines.sort_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal));
    let mut out = String::new();
    let table = detect_table(&page.vectors, &page.text);
    if let Some(tbl) = table {
        out.push_str(&tbl);
    } else {
        let mut para: Vec<&TextRun> = Vec::new();
        for line in lines {
            if para.is_empty() {
                para.push(line);
                continue;
            }
            let prev = para.last().copied().unwrap();
            let same_para = (line.y - prev.y).abs() < prev.font_size * 1.5
                && (line.x - prev.x).abs() < page.width * 0.05;
            if same_para {
                para.push(line);
            } else {
                out.push_str(&paragraph_xml(&para));
                para = vec![line];
            }
        }
        if !para.is_empty() {
            out.push_str(&paragraph_xml(&para));
        }
        for (ii, img) in page.images.iter().enumerate() {
            out.push_str(&image_inline(img, _pi, ii));
        }
    }
    let w_twips = (page.width * TWIPS_PER_PT as f64) as i64;
    let h_twips = (page.height * TWIPS_PER_PT as f64) as i64;
    out.push_str(&format!(
        r#"<w:sectPr><w:pgSz w:w="{w_twips}" w:h="{h_twips}"/><w:pgMar w:top="720" w:right="720" w:bottom="720" w:left="720"/></w:sectPr>"#
    ));
    out
}

fn paragraph_xml(runs: &[&TextRun]) -> String {
    let mut inner = String::new();
    for run in runs {
        let sz = (run.font_size * 2.0).round() as i64;
        let b = if run.bold { r#"<w:b/>"# } else { "" };
        let i = if run.italic { r#"<w:i/>"# } else { "" };
        inner.push_str(&format!(
            r#"<w:r><w:rPr><w:rFonts w:ascii="{}" w:hAnsi="{}"/><w:sz w:val="{sz}"/><w:color w:val="{}"/>{b}{i}</w:rPr><w:t>{}</w:t></w:r>"#,
            escape_xml(&run.font_key),
            escape_xml(&run.font_key),
            run.color.hex(),
            escape_xml(&run.text)
        ));
    }
    format!(
        r#"<w:p><w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>{inner}</w:p>"#
    )
}

fn image_inline(_img: &PlacedImage, _pi: usize, _ii: usize) -> String {
    String::new()
}

fn detect_table(vectors: &[VectorPath], text: &[TextRun]) -> Option<String> {
    let horiz: Vec<_> = vectors.iter().filter(|v| is_h_line(v)).collect();
    let vert: Vec<_> = vectors.iter().filter(|v| is_v_line(v)).collect();
    if horiz.len() >= 2 && vert.len() >= 2 && !text.is_empty() {
        let cell_text = escape_xml(&text[0].text);
        return Some(format!(
            r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>{cell_text}</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#
        ));
    }
    None
}

fn is_h_line(v: &VectorPath) -> bool {
    for w in v.segments.windows(2) {
        if let (crate::types::PathSegment::MoveTo(a), crate::types::PathSegment::LineTo(b)) =
            (&w[0], &w[1])
        {
            if (a.y - b.y).abs() < 1.0 && (a.x - b.x).abs() > 10.0 {
                return true;
            }
        }
    }
    false
}

fn is_v_line(v: &VectorPath) -> bool {
    for w in v.segments.windows(2) {
        if let (crate::types::PathSegment::MoveTo(a), crate::types::PathSegment::LineTo(b)) =
            (&w[0], &w[1])
        {
            if (a.x - b.x).abs() < 1.0 && (a.y - b.y).abs() > 10.0 {
                return true;
            }
        }
    }
    false
}

fn write_document_rels(
    zip: &mut ZipWriter<std::fs::File>,
    opts: SimpleFileOptions,
    fonts: &[FontEmbed],
    pages: &[ExtractedPage],
) -> Result<()> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
    );
    xml.push_str(r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#);
    xml.push_str(r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>"#);
    xml.push_str(r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/>"#);
    for (i, _font) in fonts.iter().enumerate() {
        xml.push_str(&format!(
            r#"<Relationship Id="rIdFont{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/font" Target="fonts/font{i}.odttf"/>"#
        ));
    }
    for (pi, page) in pages.iter().enumerate() {
        for (ii, img) in page.images.iter().enumerate() {
            let ext = ext_for(&img.kind);
            xml.push_str(&format!(
                r#"<Relationship Id="rIdImg{pi}_{ii}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image_p{pi}_{ii}.{ext}"/>"#
            ));
        }
    }
    xml.push_str("</Relationships>");
    zip.start_file("word/_rels/document.xml.rels", opts)
        .map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_styles(zip: &mut ZipWriter<std::fs::File>, opts: SimpleFileOptions) -> Result<()> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:default="1" w:styleId="Normal">
<w:name w:val="Normal"/>
<w:qFormat/>
<w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>
</w:style>
</w:styles>"#;
    zip.start_file("word/styles.xml", opts).map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_settings(
    zip: &mut ZipWriter<std::fs::File>,
    opts: SimpleFileOptions,
    embed_fonts: bool,
) -> Result<()> {
    let embed = if embed_fonts {
        r#"<w:embedTrueTypeFonts/>"#
    } else {
        ""
    };
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{embed}</w:settings>"#
    );
    zip.start_file("word/settings.xml", opts).map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_font_table(
    zip: &mut ZipWriter<std::fs::File>,
    opts: SimpleFileOptions,
    embed_fonts: bool,
    fonts: &[FontEmbed],
) -> Result<()> {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );
    for font in fonts {
        let embed = if embed_fonts {
            format!(
                r#"<w:embedRegular r:id="rIdFont{}"/>"#,
                fonts
                    .iter()
                    .position(|f| f.pdf_name == font.pdf_name)
                    .unwrap_or(0)
            )
        } else {
            String::new()
        };
        let _style = font.style_tag();
        xml.push_str(&format!(
            r#"<w:font w:name="{}">{embed}</w:font>"#,
            escape_xml(&font.family)
        ));
    }
    if xml.ends_with(">") && !fonts.is_empty() {
        let _ = embed_fonts;
    }
    xml.push_str("</w:fonts>");
    zip.start_file("word/fontTable.xml", opts)
        .map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_core(zip: &mut ZipWriter<std::fs::File>, opts: SimpleFileOptions) -> Result<()> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
<dc:title>Converted</dc:title>
<dc:creator>docapp</dc:creator>
</cp:coreProperties>"#;
    zip.start_file("docProps/core.xml", opts).map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

fn write_app(zip: &mut ZipWriter<std::fs::File>, opts: SimpleFileOptions) -> Result<()> {
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
<Application>docapp</Application>
</Properties>"#;
    zip.start_file("docProps/app.xml", opts).map_err(zip_err)?;
    zip.write_all(xml.as_bytes())
        .map_err(docsuite_core::Error::from)?;
    Ok(())
}

pub fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
pub fn read_zip_entry(zip_bytes: &[u8], path: &str) -> Option<Vec<u8>> {
    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor).ok()?;
    let mut file = archive.by_name(path).ok()?;
    let mut buf = Vec::new();
    std::io::copy(&mut file, &mut buf).ok()?;
    Some(buf)
}

#[cfg(test)]
pub fn xml_tag_balanced(xml: &str) -> bool {
    let mut stack: Vec<String> = Vec::new();
    let bytes = xml.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i + 1] == b'?' {
            i += 2;
            while i < bytes.len() && !(bytes[i] == b'?' && bytes[i + 1] == b'>') {
                i += 1;
            }
            i = i.saturating_add(2).min(bytes.len());
            continue;
        }
        if i + 3 < bytes.len() && &bytes[i..i + 4] == b"<!--" {
            i += 4;
            while i + 2 < bytes.len() && &bytes[i..i + 3] != b"-->" {
                i += 1;
            }
            i = i.saturating_add(3).min(bytes.len());
            continue;
        }
        let mut j = i + 1;
        let closing = bytes.get(j) == Some(&b'/');
        if closing {
            j += 1;
        }
        let name_start = j;
        while j < bytes.len() && !matches!(bytes[j], b' ' | b'>' | b'/' | b'\t' | b'\n' | b'\r') {
            j += 1;
        }
        let name = match std::str::from_utf8(&bytes[name_start..j]) {
            Ok(s) => s.to_owned(),
            Err(_) => return false,
        };
        let mut in_quote = false;
        let mut quote = b'"';
        let mut self_close = false;
        while j < bytes.len() {
            let b = bytes[j];
            if in_quote {
                if b == quote {
                    in_quote = false;
                }
            } else if b == b'"' || b == b'\'' {
                in_quote = true;
                quote = b;
            } else if b == b'/' {
                self_close = true;
            } else if b == b'>' {
                break;
            }
            j += 1;
        }
        if j >= bytes.len() {
            return false;
        }
        if closing {
            if stack.pop().as_deref() != Some(name.as_str()) {
                return false;
            }
        } else if !self_close && !name.is_empty() {
            stack.push(name);
        }
        i = j + 1;
    }
    stack.is_empty()
}

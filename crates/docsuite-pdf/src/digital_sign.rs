use std::path::Path;

use docsuite_core::{CancelToken, Error, ProgressSink, Result};
use lopdf::{IncrementalDocument, Object};
use time::OffsetDateTime;

use crate::Pdf;
use crate::cms_builder::{CmsPdfSigner, PdfSubFilter};
use crate::crop::PdfRect;
use crate::keystore::SigningIdentity;
use crate::sign_options::DigitalSignOptions;
use crate::signature::{
    finalize_signature_bytes, prepare_signature_placeholder, visible_signature,
};

pub fn sign_pdf_path(
    input: &Path,
    output: &Path,
    identity: &SigningIdentity,
    opts: &DigitalSignOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<()> {
    let bytes = std::fs::read(input).map_err(docsuite_core::Error::Io)?;
    let signed = sign_pdf_incremental(&bytes, identity, opts, cancel, progress)?;
    std::fs::write(output, signed).map_err(docsuite_core::Error::Io)
}

pub fn sign_pdf_incremental(
    input: &[u8],
    identity: &SigningIdentity,
    opts: &DigitalSignOptions,
    cancel: &CancelToken,
    progress: &Option<ProgressSink>,
) -> Result<Vec<u8>> {
    let mut inc = IncrementalDocument::create_from(
        input.to_vec(),
        lopdf::Document::load_mem(input).map_err(crate::lopdf_map::map_err)?,
    );
    if let Some((page, rect, stamp)) = &opts.visible {
        let page_id = inc
            .get_prev_documents()
            .get_pages()
            .get(page)
            .copied()
            .ok_or_else(|| Error::InvalidInput(format!("page {page} out of range")))?;
        inc.opt_clone_object_to_new_document(page_id)
            .map_err(map_inc_err)?;
        let mut pdf = Pdf::from_document(std::mem::take(&mut inc.new_document));
        visible_signature(&mut pdf, *page, *rect, stamp.clone(), cancel, progress)?;
        inc.new_document = pdf.into_document();
    }
    let m_date = pdf_m_date(OffsetDateTime::now_utc());
    let sub_filter = match opts.sub_filter {
        PdfSubFilter::Pades => "ETSI.CAdES.detached",
        PdfSubFilter::AdbePkcs7 => "adbe.pkcs7.detached",
    };
    let mut prepared = prepare_signature_placeholder(&opts.field_name, sub_filter, &m_date, opts)?;
    let sig_dict = match prepared.objects.remove(0).1 {
        Object::Dictionary(d) => d,
        _ => return Err(Error::Pdf("bad sig dict".into())),
    };
    let sig_id = inc.new_document.add_object(sig_dict);
    prepared.field_ref = sig_id;
    merge_acroform(&mut inc, &opts.field_name, sig_id)?;
    if let Some((page, rect, _)) = &opts.visible {
        add_widget(&mut inc, *page, *rect, sig_id, &opts.field_name)?;
    }
    let mut buf = Vec::new();
    inc.save_to(&mut buf).map_err(map_io_err)?;
    let signer = CmsPdfSigner {
        identity: identity.clone(),
        sub_filter: opts.sub_filter,
    };
    finalize_signature_bytes(&mut buf, &signer)
}

fn merge_acroform(
    inc: &mut IncrementalDocument,
    field_name: &str,
    sig_id: lopdf::ObjectId,
) -> Result<()> {
    let catalog_id = inc
        .new_document
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(map_inc_err)?;
    inc.opt_clone_object_to_new_document(catalog_id)
        .map_err(map_inc_err)?;
    let field = dictionary! {
        "FT" => "Sig",
        "T" => Object::string_literal(field_name.as_bytes().to_vec()),
        "V" => sig_id,
    };
    let acroform = {
        let catalog = inc
            .new_document
            .get_object(catalog_id)
            .and_then(Object::as_dict)
            .map_err(map_inc_err)?;
        catalog.get(b"AcroForm").and_then(Object::as_reference).ok()
    };
    if let Some(form_id) = acroform {
        inc.opt_clone_object_to_new_document(form_id)
            .map_err(map_inc_err)?;
        let fields_id = {
            let form = inc
                .new_document
                .get_object(form_id)
                .and_then(Object::as_dict)
                .map_err(map_inc_err)?;
            form.get(b"Fields").and_then(Object::as_reference).ok()
        };
        if let Some(fields_id) = fields_id {
            inc.opt_clone_object_to_new_document(fields_id)
                .map_err(map_inc_err)?;
            let fields = inc
                .new_document
                .get_object_mut(fields_id)
                .and_then(Object::as_array_mut)
                .map_err(map_inc_err)?;
            fields.push(Object::Dictionary(field));
        } else {
            let fields_id = inc.new_document.add_object(vec![Object::Dictionary(field)]);
            let form = inc
                .new_document
                .get_object_mut(form_id)
                .and_then(Object::as_dict_mut)
                .map_err(map_inc_err)?;
            form.set("Fields", fields_id);
        }
    } else {
        let form_id = inc.new_document.add_object(dictionary! {
            "Fields" => vec![Object::Dictionary(field)],
        });
        let catalog = inc
            .new_document
            .get_object_mut(catalog_id)
            .and_then(Object::as_dict_mut)
            .map_err(map_inc_err)?;
        catalog.set("AcroForm", form_id);
    }
    Ok(())
}

fn add_widget(
    inc: &mut IncrementalDocument,
    page: u32,
    rect: PdfRect,
    sig_id: lopdf::ObjectId,
    field_name: &str,
) -> Result<()> {
    let page_id = inc
        .get_prev_documents()
        .get_pages()
        .get(&page)
        .copied()
        .ok_or_else(|| Error::InvalidInput(format!("page {page} out of range")))?;
    inc.opt_clone_object_to_new_document(page_id)
        .map_err(map_inc_err)?;
    let widget = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "T" => Object::string_literal(field_name.as_bytes().to_vec()),
        "V" => sig_id,
        "Rect" => vec![
            Object::Real(rect.llx as f32),
            Object::Real(rect.lly as f32),
            Object::Real(rect.urx as f32),
            Object::Real(rect.ury as f32),
        ],
        "P" => page_id,
    };
    let widget_id = inc.new_document.add_object(widget);
    let page = inc
        .new_document
        .get_object_mut(page_id)
        .and_then(Object::as_dict_mut)
        .map_err(map_inc_err)?;
    if let Ok(id) = page.get(b"Annots").and_then(Object::as_reference) {
        inc.opt_clone_object_to_new_document(id)
            .map_err(map_inc_err)?;
        let arr = inc
            .new_document
            .get_object_mut(id)
            .and_then(Object::as_array_mut)
            .map_err(map_inc_err)?;
        arr.push(Object::Reference(widget_id));
    } else {
        page.set("Annots", vec![Object::Reference(widget_id)]);
    }
    Ok(())
}

fn pdf_m_date(ts: OffsetDateTime) -> String {
    format!(
        "D:{:04}{:02}{:02}{:02}{:02}{:02}+00'00'",
        ts.year(),
        u8::from(ts.month()),
        ts.day(),
        ts.hour(),
        ts.minute(),
        ts.second(),
    )
}

fn map_inc_err(e: lopdf::Error) -> Error {
    Error::Pdf(e.to_string())
}

fn map_io_err(e: std::io::Error) -> Error {
    Error::Pdf(e.to_string())
}

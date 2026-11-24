use std::collections::BTreeMap;

use docsuite_core::{CancelToken, Error, ProgressSink, Result, ThrottleState};
use lopdf::{Document, Object, ObjectId};

use crate::page_attr::materialize_page_attributes;
use crate::pdf::Pdf;
use crate::progress::step;
use crate::{lopdf_map::map_err, prune::finalize_doc};

pub fn merge(pdfs: Vec<Pdf>, cancel: &CancelToken, progress: &Option<ProgressSink>) -> Result<Pdf> {
    if pdfs.is_empty() {
        return Err(Error::InvalidInput("no documents to merge".to_owned()));
    }
    let merge_total = pdfs.len() as u64;
    let mut max_id = 1u32;
    let mut documents_pages: Vec<(ObjectId, Object)> = Vec::new();
    let mut documents_objects = BTreeMap::new();
    let mut document = Document::with_version("1.5");
    let mut state = ThrottleState::new();
    for (index, pdf) in pdfs.into_iter().enumerate() {
        step(
            cancel,
            progress,
            &mut state,
            index as u64,
            merge_total,
            "merge",
        )?;
        let mut doc = pdf.into_document();
        for page_id in doc.page_iter().collect::<Vec<_>>() {
            let page = materialize_page_attributes(&doc, page_id).map_err(map_err)?;
            doc.objects.insert(page_id, Object::Dictionary(page));
        }
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        let pages = doc.get_pages();
        for page_num in 1..=pages.len() as u32 {
            let object_id = *pages
                .get(&page_num)
                .ok_or_else(|| Error::Pdf("missing page".to_owned()))?;
            let value = doc.get_object(object_id).map_err(map_err)?.to_owned();
            documents_pages.push((object_id, value));
        }
        documents_objects.extend(doc.objects);
    }

    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                catalog_object = Some((
                    catalog_object.map(|(id, _)| id).unwrap_or(object_id),
                    object,
                ));
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object
                        && let Ok(old) = existing.as_dict()
                    {
                        dictionary.extend(old);
                    }
                    pages_object = Some((
                        pages_object.map(|(id, _)| id).unwrap_or(object_id),
                        Object::Dictionary(dictionary),
                    ));
                }
            }
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                document.objects.insert(object_id, object);
            }
        }
    }

    let Some((page_root_id, page_root)) = pages_object else {
        return Err(Error::Pdf("pages root not found".to_owned()));
    };
    let Some((catalog_id, catalog_object)) = catalog_object else {
        return Err(Error::Pdf("catalog not found".to_owned()));
    };

    for (object_id, object) in &documents_pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", page_root_id);
            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    if let Ok(dictionary) = page_root.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.remove(b"Parent");
        dictionary.remove(b"MediaBox");
        dictionary.remove(b"CropBox");
        dictionary.remove(b"Resources");
        dictionary.remove(b"Rotate");
        dictionary.set("Count", documents_pages.len() as u32);
        dictionary.set(
            "Kids",
            documents_pages
                .iter()
                .map(|(id, _)| Object::Reference(*id))
                .collect::<Vec<_>>(),
        );
        document
            .objects
            .insert(page_root_id, Object::Dictionary(dictionary));
    }

    if let Ok(dictionary) = catalog_object.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", page_root_id);
        dictionary.remove(b"Outlines");
        document
            .objects
            .insert(catalog_id, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.len() as u32;
    finalize_doc(&mut document);
    Ok(Pdf::from_document(document))
}

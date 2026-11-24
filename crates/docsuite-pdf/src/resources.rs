use lopdf::{Dictionary, Document, Object, ObjectId};

use crate::lopdf_map::map_err;
use docsuite_core::Result;

pub(crate) fn nested_dict_id(
    doc: &mut Document,
    parent_id: ObjectId,
    key: &[u8],
) -> Result<ObjectId> {
    let existing = {
        let parent = doc.get_dictionary(parent_id).map_err(map_err)?;
        match parent.get(key) {
            Ok(Object::Reference(id)) => return Ok(*id),
            Ok(Object::Dictionary(dict)) => Some(dict.clone()),
            Ok(_) => return Err(docsuite_core::Error::Pdf("expected dictionary".to_owned())),
            Err(_) => None,
        }
    };
    let id = if let Some(dict) = existing {
        let id = doc.add_object(dict);
        let parent = doc.get_dictionary_mut(parent_id).map_err(map_err)?;
        parent.set(key, id);
        id
    } else {
        let id = doc.add_object(Dictionary::new());
        let parent = doc.get_dictionary_mut(parent_id).map_err(map_err)?;
        parent.set(key, id);
        id
    };
    Ok(id)
}

pub(crate) fn page_resources_id(doc: &mut Document, page_id: ObjectId) -> Result<ObjectId> {
    doc.get_or_create_resources(page_id).map_err(map_err)?;
    let value = doc
        .get_dictionary(page_id)
        .map_err(map_err)?
        .get(b"Resources")
        .map_err(map_err)?
        .clone();
    match value {
        Object::Reference(id) => Ok(id),
        Object::Dictionary(dict) => {
            let id = doc.add_object(dict);
            let page = doc.get_dictionary_mut(page_id).map_err(map_err)?;
            page.set("Resources", id);
            Ok(id)
        }
        _ => Err(docsuite_core::Error::Pdf("bad Resources".to_owned())),
    }
}

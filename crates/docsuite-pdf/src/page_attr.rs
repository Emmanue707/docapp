use std::collections::HashSet;

use lopdf::{Dictionary, Document, Error as LopdfError, Object, ObjectId};

pub fn inherited_page_attribute(
    doc: &Document,
    page_id: ObjectId,
    key: &[u8],
) -> Result<Option<Object>, lopdf::Error> {
    let mut current = page_id;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return Err(LopdfError::ReferenceCycle(current));
        }
        let node = doc.get_dictionary(current)?;
        if let Ok(value) = node.get(key) {
            match doc.dereference(value) {
                Ok((_, Object::Null)) | Err(LopdfError::ObjectNotFound(_)) => {}
                Ok(_) => return Ok(Some(value.clone())),
                Err(error) => return Err(error),
            }
        }
        match node.get(b"Parent") {
            Err(_) | Ok(Object::Null) => return Ok(None),
            Ok(parent) => current = parent.as_reference()?,
        }
    }
}

pub fn materialize_page_attributes(
    doc: &Document,
    page_id: ObjectId,
) -> Result<Dictionary, lopdf::Error> {
    let mut page = doc.get_dictionary(page_id)?.clone();
    let media_box = inherited_page_attribute(doc, page_id, b"MediaBox")?
        .ok_or_else(|| LopdfError::DictKey("MediaBox".to_string()))?;
    let crop_box =
        inherited_page_attribute(doc, page_id, b"CropBox")?.unwrap_or_else(|| media_box.clone());
    let resources = inherited_page_attribute(doc, page_id, b"Resources")?
        .unwrap_or_else(|| Object::Dictionary(Dictionary::new()));
    let rotation = inherited_page_attribute(doc, page_id, b"Rotate")?.unwrap_or(Object::Integer(0));
    page.set("MediaBox", media_box);
    page.set("CropBox", crop_box);
    page.set("Resources", resources);
    page.set("Rotate", rotation);
    Ok(page)
}

pub fn resolve_rotate(doc: &Document, page_id: ObjectId) -> Result<i32, lopdf::Error> {
    let rotate = inherited_page_attribute(doc, page_id, b"Rotate")?.unwrap_or(Object::Integer(0));
    Ok(rotate.as_i64()? as i32)
}

pub fn mediabox_array(doc: &Document, page_id: ObjectId) -> Result<[f64; 4], lopdf::Error> {
    let value = inherited_page_attribute(doc, page_id, b"MediaBox")?
        .ok_or_else(|| LopdfError::DictKey("MediaBox".to_string()))?;
    let array = doc.dereference(&value)?.1.as_array()?;
    if array.len() != 4 {
        return Err(LopdfError::DictKey("MediaBox".to_string()));
    }
    let mut out = [0.0; 4];
    for (i, item) in array.iter().enumerate() {
        out[i] = match item {
            Object::Real(v) => f64::from(*v),
            Object::Integer(v) => *v as f64,
            _ => return Err(lopdf::Error::DictKey("MediaBox".to_string())),
        };
    }
    Ok(out)
}

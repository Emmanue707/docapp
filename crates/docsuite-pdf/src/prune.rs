use lopdf::Document;

pub(crate) fn finalize_doc(doc: &mut Document) {
    doc.prune_objects();
    doc.renumber_objects();
    doc.compress();
}

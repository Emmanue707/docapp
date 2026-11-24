use quick_xml::events::{BytesEnd, BytesStart};
use quick_xml::name::QName;

use crate::ooxf::local_name;

pub fn start_local<'a>(e: &'a BytesStart) -> &'a [u8] {
    let QName(q) = e.name();
    local_name(q)
}

pub fn end_local<'a>(e: &'a BytesEnd) -> &'a [u8] {
    let QName(q) = e.name();
    local_name(q)
}

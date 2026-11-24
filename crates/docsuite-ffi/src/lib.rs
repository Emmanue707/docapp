use std::ffi::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn docsuite_version() -> *const c_char {
    docsuite_core::version_cstr().as_ptr().cast()
}

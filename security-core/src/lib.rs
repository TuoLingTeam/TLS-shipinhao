use std::ffi::c_char;

static BACKEND_NAME: &[u8] = b"rust-security-core\0";

#[no_mangle]
pub extern "C" fn security_core_backend_name() -> *const c_char {
    BACKEND_NAME.as_ptr() as *const c_char
}


use crate::{getmodfiles, getmodslistjson};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

fn cstrtostr<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str().ok()
}

fn tocstring(s: String) -> *mut c_char {
    CString::new(s).map(|c| c.into_raw()).unwrap_or(std::ptr::null_mut())
}

fn asyncrun<F, T>(fut: F) -> Option<T>
where
    F: std::future::Future<Output = reqwest::Result<T>>,
{
    let rt = tokio::runtime::Runtime::new().ok()?;
    rt.block_on(fut).ok()
}

#[no_mangle]
pub extern "C" fn ccgetmodslistjson(query: *const c_char) -> *mut c_char {
    let query = match cstrtostr(query) {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };
    let result = asyncrun(getmodslistjson(query));
    match result {
        Some(json) => tocstring(json),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn ccgetmodfilesjson(dllink: *const c_char) -> *mut c_char {
    let dllink = match cstrtostr(dllink) {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };
    let result = asyncrun(async {
        let files = getmodfiles(dllink).await?;
        Ok(serde_json::to_string_pretty(&files).unwrap_or_default())
    });
    match result {
        Some(json) => tocstring(json),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "C" fn ccfreestring(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        let _ = CString::from_raw(s);
    }
}

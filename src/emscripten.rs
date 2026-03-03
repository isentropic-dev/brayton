//! Emscripten WASM interface.
//!
//! Exports `design_point` as an `extern "C"` function that takes a JSON string
//! and returns a JSON string.
//! The caller must free the returned string with [`free_result`].

use std::{
    ffi::{CStr, CString},
    os::raw::c_char,
};

use crate::{facade, thermo};

/// Run a design-point calculation from a JSON input string.
///
/// Returns a heap-allocated JSON string.
/// On success, the JSON matches [`facade::DesignPointOutput`].
/// On failure, returns `{"error": "..."}`.
/// The caller must free the returned pointer with [`free_result`].
#[unsafe(no_mangle)]
pub extern "C" fn design_point(input_json: *const c_char) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        let c_str = unsafe { CStr::from_ptr(input_json) };
        let json_str = c_str.to_str().unwrap_or("");

        let input: facade::DesignPointInput = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => return error_json(&format!("invalid input: {e}")),
        };

        match facade::design_point(&input) {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) => CString::new(json).unwrap().into_raw(),
                Err(e) => error_json(&format!("serialization failed: {e}")),
            },
            Err(e) => error_json(&e),
        }
    });

    match result {
        Ok(ptr) => ptr,
        Err(_) => error_json("internal panic"),
    }
}

/// Compute thermodynamic states from arrays of pressure and enthalpy.
///
/// Returns a heap-allocated JSON string.
/// On success, the JSON is an array of [`facade::StatePoint`] objects.
/// On failure, returns `{"error": "..."}`.
/// The caller must free the returned pointer with [`free_result`].
#[unsafe(no_mangle)]
pub extern "C" fn states_from_ph(input_json: *const c_char) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        let c_str = unsafe { CStr::from_ptr(input_json) };
        let json_str = c_str.to_str().unwrap_or("");

        let input: thermo::StatesFromPhInput = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => return error_json(&format!("invalid input: {e}")),
        };

        match thermo::states_from_ph(&input) {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) => CString::new(json).unwrap().into_raw(),
                Err(e) => error_json(&format!("serialization failed: {e}")),
            },
            Err(e) => error_json(&e),
        }
    });

    match result {
        Ok(ptr) => ptr,
        Err(_) => error_json("internal panic"),
    }
}

/// Compute thermodynamic states from arrays of pressure and entropy.
///
/// Returns a heap-allocated JSON string.
/// On success, the JSON is an array of [`facade::StatePoint`] objects.
/// On failure, returns `{"error": "..."}`.
/// The caller must free the returned pointer with [`free_result`].
#[unsafe(no_mangle)]
pub extern "C" fn states_from_ps(input_json: *const c_char) -> *const c_char {
    let result = std::panic::catch_unwind(|| {
        let c_str = unsafe { CStr::from_ptr(input_json) };
        let json_str = c_str.to_str().unwrap_or("");

        let input: thermo::StatesFromPsInput = match serde_json::from_str(json_str) {
            Ok(v) => v,
            Err(e) => return error_json(&format!("invalid input: {e}")),
        };

        match thermo::states_from_ps(&input) {
            Ok(output) => match serde_json::to_string(&output) {
                Ok(json) => CString::new(json).unwrap().into_raw(),
                Err(e) => error_json(&format!("serialization failed: {e}")),
            },
            Err(e) => error_json(&e),
        }
    });

    match result {
        Ok(ptr) => ptr,
        Err(_) => error_json("internal panic"),
    }
}

/// Free a string previously returned by [`design_point`].
#[unsafe(no_mangle)]
pub extern "C" fn free_result(ptr: *const c_char) {
    if !ptr.is_null() {
        unsafe {
            drop(CString::from_raw(ptr.cast_mut()));
        }
    }
}

/// Build an `{"error": "..."}` JSON response as a C string.
fn error_json(msg: &str) -> *const c_char {
    let json = format!(r#"{{"error":{}}}"#, serde_json::to_string(msg).unwrap());
    CString::new(json).unwrap().into_raw()
}

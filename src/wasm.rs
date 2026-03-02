use wasm_bindgen::prelude::*;

use crate::facade;

/// Run a simple recuperated Brayton cycle design-point calculation from JS.
///
/// Accepts a plain JS object matching [`facade::DesignPointInput`] and returns
/// a plain JS object matching [`facade::DesignPointOutput`].
#[wasm_bindgen]
pub fn design_point(input: JsValue) -> Result<JsValue, JsError> {
    let input: facade::DesignPointInput = serde_wasm_bindgen::from_value(input)?;
    let output = facade::design_point(&input).map_err(|e| JsError::new(&e))?;
    serde_wasm_bindgen::to_value(&output).map_err(|e| JsError::new(&e.to_string()))
}

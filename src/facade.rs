//! WASM/FFI-facing facades for Brayton cycle solvers.
//!
//! Each submodule wraps a cycle solver with flat, serde-friendly
//! input/output structs suitable for JSON-based interfaces.

pub mod recomp;
pub mod simple;

/// Thermodynamic state at a single cycle point.
#[derive(Debug)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct StatePoint {
    /// Temperature in degrees Celsius.
    pub temperature_c: f64,

    /// Pressure in megapascals.
    pub pressure_mpa: f64,

    /// Mass density in kilograms per cubic metre.
    pub density_kg_per_m3: f64,

    /// Specific enthalpy in kilojoules per kilogram.
    pub enthalpy_kj_per_kg: f64,

    /// Specific entropy in kilojoules per kilogram-kelvin.
    pub entropy_kj_per_kg_k: f64,
}

/// Default model name for serde deserialization.
#[cfg(feature = "wasm")]
pub(crate) fn default_fluid() -> String {
    String::from("CarbonDioxide")
}

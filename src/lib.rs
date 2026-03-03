mod config;
mod cycle;
mod error;
pub mod facade;
mod fluids;
mod operating_point;
mod solution;
// TODO: remove once non-WASM consumers use the batch helpers.
#[allow(dead_code)]
pub(crate) mod thermo;

#[cfg(feature = "wasm")]
mod emscripten;

pub use config::{
    Config, HxConfig, InvalidPressureDrop, IsentropicEfficiency, PressureDrop, RecuperatorConfig,
    TurboConfig,
};
pub use cycle::design_point;
pub use error::Error;
pub use facade::{DesignPointInput, DesignPointOutput, StatePoint};
pub use operating_point::OperatingPoint;
pub use solution::{CycleStates, Solution};

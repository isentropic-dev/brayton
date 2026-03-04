pub mod facade;
pub mod recompression;
pub mod simple;

mod config;
mod fluids;
mod operating_point;
pub(crate) mod thermo;

#[cfg(feature = "wasm")]
mod emscripten;

pub use config::{InvalidPressureDrop, IsentropicEfficiency, PressureDrop, RecuperatorConfig};
pub use facade::{
    StatePoint,
    recomp::{RecompDesignPointInput, RecompDesignPointOutput},
    simple::{DesignPointInput, DesignPointOutput},
};
pub use operating_point::OperatingPoint;

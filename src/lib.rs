pub mod facade;
pub mod simple;

mod fluids;
mod operating_point;
pub(crate) mod thermo;

#[cfg(feature = "wasm")]
mod emscripten;

pub use facade::{DesignPointInput, DesignPointOutput, StatePoint};
pub use operating_point::OperatingPoint;
pub use simple::{
    Config, CycleStates, Error, HxConfig, InvalidPressureDrop, IsentropicEfficiency, PressureDrop,
    RecuperatorConfig, Solution, TurboConfig, design_point,
};

use std::error::Error as StdError;

use thiserror::Error;
use twine_components::{
    thermal::hx::discretized,
    turbomachinery::{compressor::CompressionError, turbine::ExpansionError},
};
use twine_thermo::units::SpecificEnthalpy;

#[derive(Debug, Error)]
pub enum Error<Fluid> {
    #[error("compressor: {0}")]
    Compressor(#[from] CompressionError<Fluid>),

    #[error("turbine: {0}")]
    Turbine(#[from] ExpansionError<Fluid>),

    #[error("recuperator: {0}")]
    Recuperator(#[from] discretized::GivenUaError),

    #[error("insufficient turbine work: w_net = {w_net:?} (expected > 0)")]
    InsufficientTurbineWork { w_net: SpecificEnthalpy },

    #[error("thermo: {0}")]
    Thermo(Box<dyn StdError + Send + Sync>),
}

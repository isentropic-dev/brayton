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

    #[error(
        "pressure rise through compressor insufficient: {rise:?} less than total pressure drop of {drop:?}"
    )]
    InsufficientPressureRise {
        rise: uom::si::f64::Pressure,
        drop: uom::si::f64::Pressure,
    },

    /// A thermodynamic model operation failed.
    ///
    /// This failure can be from property evaluation or state construction.
    #[error("thermodynamic model failed: {context}")]
    ThermoModelFailed {
        /// Operation context for the thermodynamic model failure.
        context: String,

        /// Underlying thermodynamic model error.
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}

impl<Fluid> Error<Fluid> {
    /// Creates a thermo model failure error with context.
    #[allow(dead_code)]
    pub(crate) fn thermo_failed(
        context: impl Into<String>,
        err: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::ThermoModelFailed {
            context: context.into(),
            source: Box::new(err),
        }
    }
}

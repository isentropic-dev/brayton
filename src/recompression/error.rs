use std::error::Error as StdError;

use thiserror::Error;
use twine_models::support::turbomachinery::{
    compressor::CompressionError, turbine::ExpansionError,
};
use uom::si::f64::Pressure;

#[derive(Debug, Error)]
pub enum Error<Fluid> {
    /// The recompression fraction is out of range.
    ///
    /// Must satisfy `0 ≤ f < 1`.
    #[error("invalid recompression fraction: must satisfy 0 ≤ f < 1")]
    InvalidRecompressionFraction,

    /// The main compressor model failed.
    #[error("main compressor: {0}")]
    MainCompressor(CompressionError<Fluid>),

    /// The turbine model failed.
    #[error("turbine: {0}")]
    Turbine(#[from] ExpansionError<Fluid>),

    /// Net power must be positive.
    #[error("net power must be positive")]
    NonPositiveNetPower,

    /// The compressor pressure rise is smaller than the total cycle
    /// pressure drop, leaving no feasible operating point.
    #[error(
        "pressure rise through compressor insufficient: \
         rise = {rise:?}, total drop = {drop:?}"
    )]
    InsufficientPressureRise {
        /// Pressure rise across the compressor.
        rise: Pressure,
        /// Total pressure drop across all heat exchangers.
        drop: Pressure,
    },

    /// The LT recuperator solver failed to converge.
    #[error("LT recuperator solver failed: {0}")]
    LtRecuperator(String),

    /// The HT recuperator solver failed to converge.
    #[error("HT recuperator solver failed: {0}")]
    HtRecuperator(String),

    /// A thermodynamic model operation failed.
    #[error("thermodynamic model failed: {context}")]
    ThermoModelFailed {
        /// Operation context for the failure.
        context: String,

        /// Underlying error.
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
}

impl<Fluid> Error<Fluid> {
    /// Creates a thermo model failure error with context.
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

use uom::si::f64::TemperatureInterval;

use crate::config::{IsentropicEfficiency, PressureDrop, RecuperatorConfig};

/// Fixed parameters defining the recompression cycle hardware and loss models.
#[derive(Debug, Clone)]
pub struct Config {
    /// Turbomachinery efficiency parameters.
    pub turbo: TurboConfig,

    /// Heat exchanger thermal–hydraulic parameters.
    pub hx: HxConfig,

    /// Absolute temperature tolerance for the nested bisection solver.
    ///
    /// Controls convergence of the T8 (outer) and T9 (inner) iteration
    /// loops. Smaller values give tighter convergence at the cost of more
    /// iterations. A typical value is 1e-6 K.
    pub temp_tol: TemperatureInterval,
}

/// Isentropic efficiencies for the main compressor, recompressor, and turbine.
#[derive(Debug, Clone, Copy)]
pub struct TurboConfig {
    /// Main compressor isentropic efficiency.
    pub eta_mc: IsentropicEfficiency,

    /// Recompressor isentropic efficiency.
    pub eta_rc: IsentropicEfficiency,

    /// Turbine isentropic efficiency.
    pub eta_turb: IsentropicEfficiency,
}

/// Configuration for the heat exchanger models.
#[derive(Debug, Clone, Copy)]
pub struct HxConfig {
    /// Low-temperature recuperator parameters.
    pub lt_recuperator: RecuperatorConfig,

    /// High-temperature recuperator parameters.
    pub ht_recuperator: RecuperatorConfig,

    /// Precooler pressure drop.
    pub precooler_dp: PressureDrop,

    /// Primary heat exchanger pressure drop.
    pub primary_dp: PressureDrop,
}

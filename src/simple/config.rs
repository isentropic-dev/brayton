use crate::config::{IsentropicEfficiency, PressureDrop, RecuperatorConfig};

/// Fixed parameters defining the cycle hardware and loss models.
#[derive(Debug, Clone)]
pub struct Config {
    /// Turbomachinery efficiency parameters.
    pub turbo: TurboConfig,

    /// Heat exchanger thermal–hydraulic parameters.
    pub hx: HxConfig,
}

/// Isentropic efficiencies for the compressor and turbine.
#[derive(Debug, Clone, Copy)]
pub struct TurboConfig {
    /// Compressor isentropic efficiency.
    pub eta_comp: IsentropicEfficiency,

    /// Turbine isentropic efficiency.
    pub eta_turb: IsentropicEfficiency,
}

/// Configuration for the heat exchanger models.
#[derive(Debug, Clone, Copy)]
pub struct HxConfig {
    /// Recuperator parameters.
    pub recuperator: RecuperatorConfig,

    /// Precooler pressure drop.
    pub precooler_dp: PressureDrop,

    /// Primary heat exchanger pressure drop.
    pub primary_dp: PressureDrop,
}

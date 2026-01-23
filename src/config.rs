use twine_core::constraint::{
    Constrained, ConstraintResult, NonNegative, UnitIntervalLowerOpen, UnitIntervalUpperOpen,
};
use uom::si::f64::{Pressure, Ratio, ThermalConductance};

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
    pub eta_comp: Constrained<Ratio, UnitIntervalLowerOpen>,

    /// Turbine isentropic efficiency.
    pub eta_turb: Constrained<Ratio, UnitIntervalLowerOpen>,
}

/// Thermal–hydraulic parameters for heat exchangers in the cycle.
#[derive(Debug, Clone, Copy)]
pub struct HxConfig {
    /// Recuperator parameters.
    pub recup: RecuperatorConfig,

    /// Precooler pressure drop.
    pub precooler_dp: PressureDrop,

    /// Primary heat exchanger pressure drop.
    pub primary_dp: PressureDrop,
}

/// Thermal–hydraulic parameters for the recuperator model.
#[derive(Debug, Clone, Copy)]
pub struct RecuperatorConfig {
    /// Overall thermal conductance (`UA`) of the recuperator.
    pub ua: ThermalConductance,

    /// Cold-side (compressor-side) pressure drop.
    pub dp_cold: PressureDrop,

    /// Hot-side (turbine-side) pressure drop.
    pub dp_hot: PressureDrop,
}

/// Model for pressure drop across a component.
#[derive(Debug, Clone, Copy)]
pub enum PressureDrop {
    /// No pressure drop.
    None,

    /// Fixed pressure drop `Δp`.
    Absolute(Constrained<Pressure, NonNegative>),

    /// Fractional pressure drop `f` referenced to inlet pressure (`Δp = f · p_in`).
    Fraction(Constrained<Ratio, UnitIntervalUpperOpen>),
}

impl PressureDrop {
    /// Construct a fixed pressure drop `Δp`.
    ///
    /// # Errors
    ///
    /// Returns an error if `dp` is negative.
    pub fn absolute(dp: Pressure) -> ConstraintResult<Self> {
        Ok(Self::Absolute(Constrained::new(dp)?))
    }

    /// Construct a fractional pressure drop `f` referenced to inlet pressure.
    ///
    /// # Errors
    ///
    /// Returns an error if `f` is not in the interval `0 ≤ f < 1`.
    pub fn fraction(f: Ratio) -> ConstraintResult<Self> {
        Ok(Self::Fraction(Constrained::new(f)?))
    }
}

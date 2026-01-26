use twine_components::thermal::hx::discretized::GivenUaConfig;
use twine_core::constraint::{
    Constrained, ConstraintResult, NonNegative, UnitBounds, UnitInterval, UnitIntervalLowerOpen,
    UnitIntervalUpperOpen,
};
use uom::si::{
    f64::{Pressure, Ratio, ThermalConductance},
    ratio::ratio,
};

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

/// Isentropic efficiency for turbomachinery components.
#[derive(Debug, Clone, Copy)]
pub struct IsentropicEfficiency(Constrained<Ratio, UnitIntervalLowerOpen>);

impl IsentropicEfficiency {
    /// Constructs from a dimensionless ratio value.
    ///
    /// # Errors
    ///
    /// Returns an error if `value` is not in the interval (0, 1].
    pub fn new(value: f64) -> ConstraintResult<Self> {
        Ok(Self(Constrained::new(Ratio::new::<ratio>(value))?))
    }

    /// Constructs from a `Ratio` (e.g., using percent: `Ratio::new::<percent>(85.0)`).
    ///
    /// # Errors
    ///
    /// Returns an error if `r` is not in (0, 1] when expressed as a dimensionless ratio.
    pub fn from_ratio(r: Ratio) -> ConstraintResult<Self> {
        Ok(Self(Constrained::new(r)?))
    }

    /// Returns the efficiency as a `Ratio`.
    #[must_use]
    pub fn as_ratio(&self) -> Ratio {
        *self.0.as_ref()
    }

    /// Returns the efficiency as `Constrained<Ratio, UnitIntervalLowerOpen>`.
    ///
    /// Use this for compressor isentropic efficiency.
    #[must_use]
    pub fn as_lower_open(&self) -> Constrained<Ratio, UnitIntervalLowerOpen> {
        self.0
    }

    /// Returns the efficiency as `Constrained<Ratio, UnitInterval>`.
    ///
    /// Use this for turbine isentropic efficiency.
    /// The widening from (0, 1] to [0, 1] is safe because (0, 1] ⊆ [0, 1].
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn as_unit_interval(&self) -> Constrained<Ratio, UnitInterval> {
        Constrained::new(self.0.into_inner())
            .expect("UnitIntervalLowerOpen is within UnitInterval bounds")
    }
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

/// Configuration for the recuperator model.
#[derive(Debug, Clone, Copy)]
pub struct RecuperatorConfig {
    /// Overall thermal conductance (`UA`) of the recuperator.
    pub ua: Constrained<ThermalConductance, NonNegative>,

    /// Cold-side (compressor-side) pressure drop.
    pub dp_cold: PressureDrop,

    /// Hot-side (turbine-side) pressure drop.
    pub dp_hot: PressureDrop,

    /// Convergence settings for the recuperator solver.
    pub convergence: GivenUaConfig,
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
    /// Constructs a fixed pressure drop `Δp`.
    ///
    /// # Errors
    ///
    /// Returns an error if `dp` is negative.
    pub fn absolute(dp: Pressure) -> ConstraintResult<Self> {
        Ok(Self::Absolute(Constrained::new(dp)?))
    }

    /// Constructs a fractional pressure drop `f` referenced to inlet pressure.
    ///
    /// # Errors
    ///
    /// Returns an error if `f` is not in the interval `0 ≤ f < 1`.
    pub fn fraction(f: Ratio) -> ConstraintResult<Self> {
        Ok(Self::Fraction(Constrained::new(f)?))
    }

    /// Calculates outlet pressure given inlet pressure.
    ///
    /// For forward flow direction: `p_out = p_in - Δp`
    #[must_use]
    pub fn outlet_pressure(&self, p_inlet: Pressure) -> Pressure {
        match self {
            Self::None => p_inlet,
            Self::Absolute(dp) => p_inlet - *dp.as_ref(),
            Self::Fraction(f) => p_inlet * (Ratio::one() - *f.as_ref()),
        }
    }

    /// Calculates inlet pressure given outlet pressure.
    ///
    /// For backward flow direction: `p_in = p_out + Δp`
    ///
    /// For fractional drops: `Δp = f · p_in`, so solving gives `p_in = p_out / (1 - f)`
    #[must_use]
    pub fn inlet_pressure(&self, p_outlet: Pressure) -> Pressure {
        match self {
            Self::None => p_outlet,
            Self::Absolute(dp) => p_outlet + *dp.as_ref(),
            Self::Fraction(f) => p_outlet / (Ratio::one() - *f.as_ref()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use approx::assert_relative_eq;
    use uom::si::{
        f64::{Pressure, Ratio},
        pressure::kilopascal,
        ratio::{percent, ratio},
    };

    #[test]
    fn isentropic_efficiency_new_and_as_ratio() {
        let eta = IsentropicEfficiency::new(0.85).unwrap();
        assert_relative_eq!(eta.as_ratio().value, 0.85);
    }

    #[test]
    fn isentropic_efficiency_from_ratio_percent() {
        let eta = IsentropicEfficiency::from_ratio(Ratio::new::<percent>(85.0)).unwrap();
        assert_relative_eq!(eta.as_ratio().value, 0.85);
    }

    #[test]
    fn isentropic_efficiency_validates_bounds() {
        assert!(IsentropicEfficiency::new(0.0).is_err());
        assert!(IsentropicEfficiency::new(1.5).is_err());
        assert!(IsentropicEfficiency::new(0.85).is_ok());
    }

    #[test]
    fn pressure_drop_absolute_round_trip() {
        let p_inlet = Pressure::new::<kilopascal>(80.0);
        let dp = PressureDrop::absolute(Pressure::new::<kilopascal>(2.5)).unwrap();

        let p_outlet = dp.outlet_pressure(p_inlet);
        let p_back = dp.inlet_pressure(p_outlet);

        assert_eq!(p_back, p_inlet);
    }

    #[test]
    fn pressure_drop_fraction_round_trip() {
        let p_inlet = Pressure::new::<kilopascal>(20.0);
        let dp = PressureDrop::fraction(Ratio::new::<ratio>(0.02)).unwrap();

        let p_outlet = dp.outlet_pressure(p_inlet);
        let p_back = dp.inlet_pressure(p_outlet);

        assert_eq!(p_back, p_inlet);
    }

    #[test]
    fn pressure_drop_fraction_outlet_math() {
        let p_inlet = Pressure::new::<kilopascal>(50.0);
        let dp = PressureDrop::fraction(Ratio::new::<ratio>(0.1)).unwrap();

        let expected = Pressure::new::<kilopascal>(45.0);
        let actual = dp.outlet_pressure(p_inlet);

        assert_eq!(actual, expected);
    }
}

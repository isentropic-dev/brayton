//! Correct recuperator effectiveness via pinch-point analysis.
//!
//! The effectiveness values reported by `twine-models` are incorrect for
//! variable-property fluids (see [twine-models#73]).
//! This module computes effectiveness by finding the maximum possible
//! heat transfer — the point where an infinitely long counterflow HX
//! would develop a zero temperature pinch — and dividing actual heat
//! transfer by that limit.
//!
//! [twine-models#73]: https://github.com/isentropic-dev/twine-models/issues/73

use std::convert::Infallible;

use twine_core::EquationProblem;
use twine_models::{
    models::thermal::hx::discretized::{
        Inlets, MassFlows, OutletTemp, PressureDrops, RecuperatorGivenOutlet,
        RecuperatorGivenOutletInput, RecuperatorGivenOutletOutput,
    },
    support::thermo::capability::{HasEnthalpy, HasPressure, StateFrom, ThermoModel},
    support::units::SpecificEnthalpy,
};
use twine_solvers::equation::bisection;
use uom::si::{
    f64::{Power, Pressure, ThermodynamicTemperature},
    power::watt,
    temperature_interval::kelvin as delta_kelvin,
    thermodynamic_temperature::kelvin,
};

/// Maximum bisection iterations for finding the pinch-point limit.
const MAX_ITERS: usize = 100;

/// Temperature tolerance (K) for the bisection solver.
/// Convergence is declared when the bracket width drops below this.
const TEMP_TOL_K: f64 = 1e-6;

/// Small offset (K) subtracted from `min_delta_t` in the residual.
/// Ensures the residual crosses zero just before the true pinch, where
/// `min_delta_t` reaches zero but never goes negative.
const RESIDUAL_OFFSET_K: f64 = 1e-8;

/// Trait alias for the thermo model bounds needed by effectiveness computation.
///
/// Same bounds that `RecuperatorGivenOutlet` requires.
pub trait EffectivenessThermo<Fluid>:
    ThermoModel<Fluid = Fluid>
    + HasPressure
    + HasEnthalpy
    + StateFrom<(Fluid, ThermodynamicTemperature, Pressure)>
    + StateFrom<(Fluid, Pressure, SpecificEnthalpy)>
{
}

impl<Fluid, T> EffectivenessThermo<Fluid> for T where
    T: ThermoModel<Fluid = Fluid>
        + HasPressure
        + HasEnthalpy
        + StateFrom<(Fluid, ThermodynamicTemperature, Pressure)>
        + StateFrom<(Fluid, Pressure, SpecificEnthalpy)>
{
}

/// Computes recuperator effectiveness by finding the pinch-point limit.
///
/// Uses bisection on the cold-side outlet temperature to find `q_max`
/// (the heat transfer where `min_delta_t` reaches zero), then returns
/// `q_actual / q_max`.
///
/// Returns `None` if the bisection or recuperator model fails.
pub fn compute<Fluid, Thermo>(
    inlets: Inlets<Fluid, Fluid>,
    mass_flows: MassFlows,
    pressure_drops: PressureDrops,
    q_actual: Power,
    segments: usize,
    thermo: &Thermo,
) -> Option<f64>
where
    Fluid: Clone,
    Thermo: EffectivenessThermo<Fluid>,
{
    let q_actual_w = q_actual.get::<watt>();
    if q_actual_w <= 0.0 {
        return Some(0.0);
    }

    let recup = RecuperatorGivenOutlet::new(thermo, segments).ok()?;

    // Bracket: cold outlet ranges from cold inlet (no heat transfer)
    // to hot inlet (maximum possible — may violate second law).
    let t_cold_in = inlets.top.temperature.get::<kelvin>();
    let t_hot_in = inlets.bottom.temperature.get::<kelvin>();

    let problem = PinchProblem {
        inlets,
        mass_flows,
        pressure_drops,
    };

    let config = bisection::Config {
        max_iters: MAX_ITERS,
        x_abs_tol: TEMP_TOL_K,
        x_rel_tol: 0.0,
        residual_tol: 0.0,
    };

    // The upper bracket endpoint (hot inlet temperature) may cause the
    // model to fail for real-gas fluids (second-law violation). Since
    // `solve` doesn't use the observer for initial bracket evaluation,
    // we use `solve_from_bracket` with assumed signs: positive at the
    // cold inlet (large min_delta_t) and negative at the hot inlet
    // (pinch violated or zero).
    let bracket = bisection::Bracket::new(
        (t_cold_in, bisection::Sign::Positive),
        (t_hot_in, bisection::Sign::Negative),
    )
    .ok()?;

    // When the model fails during iteration, assume positive residual —
    // the outlet temperature is too high.
    let observer = |event: &bisection::Event<'_, _, _>| match event {
        bisection::Event::Evaluated { .. } => None,
        bisection::Event::ModelFailed { .. } | bisection::Event::ProblemFailed { .. } => {
            Some(bisection::Action::assume_positive())
        }
    };

    let solution =
        bisection::solve_from_bracket(&recup, &problem, bracket, &config, observer).ok()?;

    let q_max_w = solution.snapshot.output.q_dot.magnitude().get::<watt>();
    // Near-zero q_max implies near-zero q_actual (can't transfer more
    // heat than the pinch-point limit), so the ratio stays bounded.
    // The clamp below handles any residual numerical overshoot.
    if q_max_w <= 0.0 {
        return Some(0.0);
    }

    Some((q_actual_w / q_max_w).clamp(0.0, 1.0))
}

/// Equation problem that drives `min_delta_t` to zero.
///
/// The solver variable is the cold-side outlet temperature in kelvin.
struct PinchProblem<Fluid> {
    inlets: Inlets<Fluid, Fluid>,
    mass_flows: MassFlows,
    pressure_drops: PressureDrops,
}

impl<Fluid: Clone> EquationProblem<1> for PinchProblem<Fluid> {
    type Input = RecuperatorGivenOutletInput<Fluid>;
    type Output = RecuperatorGivenOutletOutput<Fluid>;
    type Error = Infallible;

    fn input(&self, x: &[f64; 1]) -> Result<Self::Input, Self::Error> {
        Ok(RecuperatorGivenOutletInput {
            inlets: self.inlets.clone(),
            mass_flows: self.mass_flows,
            pressure_drops: self.pressure_drops,
            outlet_temp: OutletTemp::Top(ThermodynamicTemperature::new::<kelvin>(x[0])),
        })
    }

    fn residuals(
        &self,
        _input: &Self::Input,
        output: &Self::Output,
    ) -> Result<[f64; 1], Self::Error> {
        // Offset by a small tolerance so the residual crosses zero just
        // before the true pinch.
        // Without this, min_delta_t goes from positive to zero but never
        // negative, and bisection can't find a sign change.
        Ok([output.min_delta_t.value.get::<delta_kelvin>() - RESIDUAL_OFFSET_K])
    }
}

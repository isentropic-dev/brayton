// mc (main compressor) and rc (recompressor) are standard domain abbreviations.
// `similar_names` is allowed on specific functions rather than module-wide.

use std::marker::PhantomData;

use twine_core::{EquationProblem, Model};
use twine_models::{
    models::thermal::hx::discretized::{
        Inlets, MassFlows, OutletTemp, PressureDrops, RecuperatorGivenOutlet,
        RecuperatorGivenOutletError, RecuperatorGivenOutletInput, RecuperatorGivenOutletOutput,
    },
    support::{
        thermo::{
            State,
            capability::{HasEnthalpy, HasEntropy, HasPressure, StateFrom, ThermoModel},
        },
        turbomachinery::{compressor, turbine},
        units::{SpecificEnthalpy, SpecificEntropy},
    },
};
use twine_solvers::equation::bisection::{
    self, Action, Bracket, Config as BisectionConfig, Event, Sign,
};
use uom::{
    ConstZero,
    si::{
        f64::{MassRate, Pressure, Ratio, ThermalConductance, ThermodynamicTemperature},
        ratio::ratio,
        thermal_conductance::watt_per_kelvin,
        thermodynamic_temperature::kelvin,
    },
};

use super::{
    config::Config,
    error::Error,
    solution::{CycleStates, Solution},
};
use crate::{OperatingPoint, config::IsentropicEfficiency};

/// Solve for cycle performance at the design point.
///
/// Calculates all thermodynamic states, mass flow rates, component powers,
/// and thermal efficiency for a recompression Brayton cycle.
///
/// The solver uses nested bisection: outer on T8 (HT recuperator hot outlet)
/// and inner on T9 (LT recuperator hot outlet). Both use forward UA
/// calculations via `RecuperatorGivenOutlet` and drive the residual
/// `UA_calc − UA_target` to zero.
///
/// # Errors
///
/// Returns an error on invalid inputs, infeasible operating points,
/// thermodynamic model failures, or solver convergence failures.
pub fn design_point<Fluid, Thermo>(
    operating_point: OperatingPoint,
    recomp_frac: Ratio,
    config: &Config,
    fluid: Fluid,
    thermo: &Thermo,
) -> Result<Solution<Fluid>, Error<Fluid>>
where
    Fluid: Clone,
    Thermo: ThermoModel<Fluid = Fluid>
        + HasPressure
        + HasEnthalpy
        + HasEntropy
        + StateFrom<(Fluid, ThermodynamicTemperature, Pressure)>
        + StateFrom<(Fluid, Pressure, SpecificEnthalpy)>
        + StateFrom<(Fluid, Pressure, SpecificEntropy)>,
{
    // Validate inputs.
    let f = recomp_frac.get::<ratio>();
    if !(0.0..1.0).contains(&f) {
        return Err(Error::InvalidRecompressionFraction);
    }
    if operating_point.net_power.value <= 0.0 {
        return Err(Error::NonPositiveNetPower);
    }

    let pressures = CyclePressures::new(&operating_point, config)?;

    // Calculate known states from the operating point.
    let s1 = thermo
        .state_from((fluid.clone(), operating_point.t_comp_in, pressures.p1))
        .map_err(|e| Error::thermo_failed("state_from(compressor inlet)", e))?;
    let s6 = thermo
        .state_from((fluid.clone(), operating_point.t_turb_in, pressures.p6))
        .map_err(|e| Error::thermo_failed("state_from(turbine inlet)", e))?;

    // Main compressor: s1 → s2.
    let mc_result = compressor::isentropic(&s1, pressures.p2, config.turbo.eta_mc, thermo)
        .map_err(Error::MainCompressor)?;
    let s2 = mc_result.outlet;
    let mc_work = mc_result.work.quantity();

    // Turbine: s6 → s7.
    let turb_result = turbine::isentropic(&s6, pressures.p7, config.turbo.eta_turb, thermo)?;
    let s7 = turb_result.outlet;
    let turb_work = turb_result.work.quantity();

    // Get T2 and T7 in kelvin for bracket bounds.
    let t2_k = s2.temperature.get::<kelvin>();
    let t7_k = s7.temperature.get::<kelvin>();

    // Construct recuperator models.
    let lt_recup = RecuperatorGivenOutlet::new(thermo, config.hx.lt_recuperator.segments)
        .map_err(|e| Error::LtRecuperator(e.to_string()))?;
    let ht_recup = RecuperatorGivenOutlet::new(thermo, config.hx.ht_recuperator.segments)
        .map_err(|e| Error::HtRecuperator(e.to_string()))?;

    // Absolute pressure drops for recuperator inputs.
    let lt_dp =
        PressureDrops::new_unchecked(pressures.p2 - pressures.p3, pressures.p8 - pressures.p9);
    let ht_dp =
        PressureDrops::new_unchecked(pressures.p4 - pressures.p5, pressures.p7 - pressures.p8);

    // Bisection config derived from the single temperature tolerance knob.
    let temp_tol_k = config
        .temp_tol
        .get::<uom::si::temperature_interval::kelvin>();
    let bisection_config = BisectionConfig {
        max_iters: 100,
        x_abs_tol: temp_tol_k,
        x_rel_tol: 0.0,
        residual_tol: 0.0,
    };

    // Build the context struct shared by both models.
    let ctx = SolverContext {
        fluid: &fluid,
        thermo,
        recomp_frac: f,
        mc_work,
        turb_work,
        net_power: operating_point.net_power,
        s2: &s2,
        s7: &s7,
        p4: pressures.p4,
        p8: pressures.p8,
        p9: pressures.p9,
        p10: pressures.p10,
        lt_recup: &lt_recup,
        ht_recup: &ht_recup,
        lt_dp,
        ht_dp,
        lt_ua_target: config.hx.lt_recuperator.ua,
        eta_rc: config.turbo.eta_rc,
        bisection_config: &bisection_config,
    };

    // Outer bisection on T8.
    //
    // Bracket: [T2, T7].
    // At T8 = T2: HT recup hot outlet equals cold inlet → max ΔT → UA_calc >> UA_target → positive.
    // At T8 = T7: HT recup hot outlet equals hot inlet → zero heat transfer → UA_calc = 0 → negative.
    let outer_bracket = Bracket::new((t2_k, Sign::Positive), (t7_k, Sign::Negative))
        .map_err(|e| Error::HtRecuperator(format!("invalid bracket: {e}")))?;

    let outer_model = OuterModel { ctx: &ctx };
    let outer_problem = OuterProblem::<Fluid> {
        ua_target: config.hx.ht_recuperator.ua.get::<watt_per_kelvin>(),
        _fluid: PhantomData,
    };

    let outer_observer =
        |event: &Event<'_, OuterModel<'_, '_, Fluid, Thermo>, OuterProblem<Fluid>>| match event {
            Event::ModelFailed { error, .. } if error.is_second_law_violation() => {
                Some(Action::assume_positive())
            }
            _ => None,
        };

    let outer_solution = bisection::solve_from_bracket(
        &outer_model,
        &outer_problem,
        outer_bracket,
        &bisection_config,
        outer_observer,
    )
    .map_err(|e| Error::HtRecuperator(format!("bisection failed: {e}")))?;

    let known = KnownStatesAndWork {
        s1,
        s2,
        s6,
        s7,
        mc_work,
        turb_work,
        f,
    };
    assemble_solution(outer_solution.snapshot.output, known, thermo)
}

/// States and specific work values established before the bisection.
struct KnownStatesAndWork<Fluid> {
    s1: State<Fluid>,
    s2: State<Fluid>,
    s6: State<Fluid>,
    s7: State<Fluid>,
    mc_work: SpecificEnthalpy,
    turb_work: SpecificEnthalpy,
    f: f64,
}

/// Build the [`Solution`] from converged bisection outputs.
#[allow(clippy::similar_names)] // mc/rc/lt/ht are standard domain abbreviations.
fn assemble_solution<Fluid, Thermo>(
    outer: OuterOutput<Fluid>,
    known: KnownStatesAndWork<Fluid>,
    thermo: &Thermo,
) -> Result<Solution<Fluid>, Error<Fluid>>
where
    Fluid: Clone,
    Thermo: HasEnthalpy<Fluid = Fluid>,
{
    let inner = outer.inner;
    let KnownStatesAndWork {
        s1,
        s2,
        s6,
        s7,
        mc_work,
        turb_work,
        f,
    } = known;

    // Retrieve enthalpies for heat balance calculations.
    let h1 = thermo
        .enthalpy(&s1)
        .map_err(|e| Error::thermo_failed("enthalpy(s1)", e))?;
    let h5 = thermo
        .enthalpy(&outer.ht_result.top_outlet)
        .map_err(|e| Error::thermo_failed("enthalpy(s5)", e))?;
    let h6 = thermo
        .enthalpy(&s6)
        .map_err(|e| Error::thermo_failed("enthalpy(s6)", e))?;
    let h9 = thermo
        .enthalpy(&inner.s9)
        .map_err(|e| Error::thermo_failed("enthalpy(s9)", e))?;

    let m_dot_t = inner.m_dot_t;
    let m_dot_mc = inner.m_dot_mc;
    let m_dot_rc = inner.m_dot_rc;

    // Component powers.
    let w_dot_mc = mc_work * m_dot_mc;
    let w_dot_rc = inner.rc_work * m_dot_rc;
    let w_dot_turb = turb_work * m_dot_t;

    // Heat rates.
    let q_dot_phx = (h6 - h5) * m_dot_t;
    let q_dot_pc = (h9 - h1) * m_dot_mc;
    let q_dot_lt = inner.lt_result.q_dot.magnitude();
    let q_dot_ht = outer.ht_result.q_dot.magnitude();

    // Thermal efficiency.
    let w_net = turb_work - mc_work * (1.0 - f) - inner.rc_work * f;
    let q_phx = h6 - h5;
    let eta_thermal = w_net / q_phx;

    Ok(Solution {
        states: CycleStates {
            s1,
            s2,
            s3: inner.lt_result.top_outlet,
            s4: inner.s4,
            s5: outer.ht_result.top_outlet,
            s6,
            s7,
            s8: outer.ht_result.bottom_outlet,
            s9: inner.s9,
            s10: inner.s10,
        },
        m_dot_t,
        m_dot_mc,
        m_dot_rc,
        w_dot_mc,
        w_dot_rc,
        w_dot_turb,
        q_dot_phx,
        q_dot_pc,
        q_dot_lt,
        q_dot_ht,
        eta_thermal,
        lt_min_delta_t: inner.lt_result.min_delta_t.value,
        ht_min_delta_t: outer.ht_result.min_delta_t.value,
        lt_effectiveness: inner.lt_result.effectiveness,
        ht_effectiveness: outer.ht_result.effectiveness,
    })
}

// ---------------------------------------------------------------------------
// Solver internals
// ---------------------------------------------------------------------------

/// Pressures at each state point, computed from the operating point and config.
struct CyclePressures {
    p1: Pressure,
    p2: Pressure,
    p3: Pressure,
    p4: Pressure,
    p5: Pressure,
    p6: Pressure,
    p7: Pressure,
    p8: Pressure,
    p9: Pressure,
    p10: Pressure,
}

impl CyclePressures {
    /// Compute all cycle pressures from operating conditions and pressure drops.
    ///
    /// High side (forward from compressor outlet P2):
    ///   `P3 = P2 − dp_lt_cold`, `P4 = P10 = P3`, `P5 = P4 − dp_ht_cold`, `P6 = P5 − dp_phx`.
    ///
    /// Low side (backward from precooler outlet P1):
    ///   `P9 = P1 + dp_pc`, `P8 = P9 + dp_lt_hot`, `P7 = P8 + dp_ht_hot`.
    fn new<Fluid>(op: &OperatingPoint, config: &Config) -> Result<Self, Error<Fluid>> {
        let p1 = op.p_comp_in;
        let p2 = op.p_comp_out;
        let p3 = config.hx.lt_recuperator.dp_cold.outlet_pressure(p2);
        let p4 = p3;
        let p10 = p3;
        let p5 = config.hx.ht_recuperator.dp_cold.outlet_pressure(p4);
        let p6 = config.hx.primary_dp.outlet_pressure(p5);

        let p9 = config.hx.precooler_dp.inlet_pressure(p1);
        let p8 = config.hx.lt_recuperator.dp_hot.inlet_pressure(p9);
        let p7 = config.hx.ht_recuperator.dp_hot.inlet_pressure(p8);

        if p6 <= p7 {
            return Err(Error::InsufficientPressureRise {
                rise: p2 - p1,
                drop: (p2 - p6) + (p7 - p1),
            });
        }

        Ok(Self {
            p1,
            p2,
            p3,
            p4,
            p5,
            p6,
            p7,
            p8,
            p9,
            p10,
        })
    }
}

/// Shared context for the nested bisection.
struct SolverContext<'a, Fluid, Thermo> {
    fluid: &'a Fluid,
    thermo: &'a Thermo,
    recomp_frac: f64,
    mc_work: SpecificEnthalpy,
    turb_work: SpecificEnthalpy,
    net_power: uom::si::f64::Power,
    s2: &'a State<Fluid>,
    s7: &'a State<Fluid>,
    p4: Pressure,
    p8: Pressure,
    p9: Pressure,
    p10: Pressure,
    lt_recup: &'a RecuperatorGivenOutlet<Fluid, &'a Thermo>,
    ht_recup: &'a RecuperatorGivenOutlet<Fluid, &'a Thermo>,
    lt_dp: PressureDrops,
    ht_dp: PressureDrops,
    lt_ua_target: ThermalConductance,
    eta_rc: IsentropicEfficiency,
    bisection_config: &'a BisectionConfig,
}

// -- Inner bisection (T9) --------------------------------------------------

/// Output from the inner model (one evaluation at a given T9).
#[derive(Debug, Clone)]
struct InnerOutput<Fluid> {
    /// LT recuperator results.
    lt_result: RecuperatorGivenOutletOutput<Fluid>,

    /// State 9 (LT hot outlet / recompressor inlet).
    s9: State<Fluid>,

    /// State 10 (recompressor outlet).
    s10: State<Fluid>,

    /// State 4 (mixing valve outlet).
    s4: State<Fluid>,

    /// Turbine mass flow rate.
    m_dot_t: MassRate,

    /// Main compressor mass flow rate.
    m_dot_mc: MassRate,

    /// Recompressor mass flow rate.
    m_dot_rc: MassRate,

    /// Recompressor specific work.
    rc_work: SpecificEnthalpy,
}

/// Error from the inner model.
#[derive(Debug, thiserror::Error)]
enum InnerError {
    #[error("second law violation")]
    SecondLawViolation,

    #[error("{0}")]
    Other(String),
}

impl InnerError {
    fn is_second_law_violation(&self) -> bool {
        matches!(self, Self::SecondLawViolation)
    }
}

/// Inner model: given T9 (in kelvin), compute LT recuperator forward UA
/// and all intermediate states needed by the outer model.
struct InnerModel<'a, 'ctx, Fluid, Thermo> {
    ctx: &'a SolverContext<'ctx, Fluid, Thermo>,
    t8_k: f64,
}

impl<Fluid, Thermo> Model for InnerModel<'_, '_, Fluid, Thermo>
where
    Fluid: Clone,
    Thermo: ThermoModel<Fluid = Fluid>
        + HasPressure
        + HasEnthalpy
        + HasEntropy
        + StateFrom<(Fluid, ThermodynamicTemperature, Pressure)>
        + StateFrom<(Fluid, Pressure, SpecificEnthalpy)>
        + StateFrom<(Fluid, Pressure, SpecificEntropy)>,
{
    type Input = f64;
    type Output = InnerOutput<Fluid>;
    type Error = InnerError;

    #[allow(clippy::similar_names)] // mc/rc are standard domain abbreviations.
    fn call(&self, input: &f64) -> Result<InnerOutput<Fluid>, InnerError> {
        let t9_k = *input;
        let ctx = self.ctx;
        let f = ctx.recomp_frac;

        // Construct state 9 from (T9, P9).
        let s9 = ctx
            .thermo
            .state_from((
                ctx.fluid.clone(),
                ThermodynamicTemperature::new::<kelvin>(t9_k),
                ctx.p9,
            ))
            .map_err(|e| InnerError::Other(format!("state_from(s9): {e}")))?;

        // Recompressor: s9 → s10.
        let rc_result = compressor::isentropic(&s9, ctx.p10, ctx.eta_rc, ctx.thermo)
            .map_err(|e| InnerError::Other(format!("recompressor: {e}")))?;
        let s10 = rc_result.outlet;
        let rc_work = rc_result.work.quantity();

        // Compute mass flow rates from energy balance.
        // W_dot_net = m_dot_t * (w_turb − w_mc*(1−f) − w_rc*f)
        let w_net = ctx.turb_work - ctx.mc_work * (1.0 - f) - rc_work * f;
        if w_net <= SpecificEnthalpy::ZERO {
            return Err(InnerError::Other("insufficient net work".to_string()));
        }
        let m_dot_t = ctx.net_power / w_net;
        let m_dot_mc = m_dot_t * (1.0 - f);
        let m_dot_rc = m_dot_t * f;

        // Construct state 8 from (T8, P8) — LT recuperator hot inlet.
        let s8 = ctx
            .thermo
            .state_from((
                ctx.fluid.clone(),
                ThermodynamicTemperature::new::<kelvin>(self.t8_k),
                ctx.p8,
            ))
            .map_err(|e| InnerError::Other(format!("state_from(s8): {e}")))?;

        // LT recuperator forward solve.
        // Top = cold side (main compressor flow: s2 → s3).
        // Bottom = hot side (total flow: s8 → s9).
        // Known outlet: bottom at T9.
        let lt_result = ctx
            .lt_recup
            .call(&RecuperatorGivenOutletInput {
                inlets: Inlets {
                    top: ctx.s2.clone(),
                    bottom: s8,
                },
                mass_flows: MassFlows::new_unchecked(m_dot_mc, m_dot_t),
                pressure_drops: ctx.lt_dp,
                outlet_temp: OutletTemp::Bottom(ThermodynamicTemperature::new::<kelvin>(t9_k)),
            })
            .map_err(|e| match &e {
                RecuperatorGivenOutletError::SecondLawViolation { .. } => {
                    InnerError::SecondLawViolation
                }
                _ => InnerError::Other(format!("LT recuperator: {e}")),
            })?;

        // Mixing valve: h4 = (1−f)·h3 + f·h10.
        let h3 = ctx
            .thermo
            .enthalpy(&lt_result.top_outlet)
            .map_err(|e| InnerError::Other(format!("enthalpy(s3): {e}")))?;
        let h10 = ctx
            .thermo
            .enthalpy(&s10)
            .map_err(|e| InnerError::Other(format!("enthalpy(s10): {e}")))?;
        let h4 = h3 * (1.0 - f) + h10 * f;

        let s4 = ctx
            .thermo
            .state_from((ctx.fluid.clone(), ctx.p4, h4))
            .map_err(|e| InnerError::Other(format!("state_from(s4): {e}")))?;

        Ok(InnerOutput {
            lt_result,
            s9,
            s10,
            s4,
            m_dot_t,
            m_dot_mc,
            m_dot_rc,
            rc_work,
        })
    }
}

/// Inner problem: residual = `UA_LT_calc` − `UA_LT_target`.
struct InnerProblem<Fluid> {
    ua_target: f64,
    _fluid: PhantomData<Fluid>,
}

impl<Fluid> EquationProblem<1> for InnerProblem<Fluid> {
    type Input = f64;
    type Output = InnerOutput<Fluid>;
    type Error = std::convert::Infallible;

    fn input(&self, x: &[f64; 1]) -> Result<f64, Self::Error> {
        Ok(x[0])
    }

    fn residuals(
        &self,
        _input: &f64,
        output: &InnerOutput<Fluid>,
    ) -> Result<[f64; 1], Self::Error> {
        let ua_calc = output.lt_result.ua.get::<watt_per_kelvin>();
        Ok([ua_calc - self.ua_target])
    }
}

// -- Outer bisection (T8) --------------------------------------------------

/// Output from the outer model.
#[derive(Debug, Clone)]
struct OuterOutput<Fluid> {
    /// HT recuperator results.
    ht_result: RecuperatorGivenOutletOutput<Fluid>,

    /// Inner solution (from the converged T9 bisection).
    inner: InnerOutput<Fluid>,
}

/// Error from the outer model.
#[derive(Debug, thiserror::Error)]
enum OuterError {
    #[error("second law violation")]
    SecondLawViolation,

    #[error("{0}")]
    Other(String),
}

impl OuterError {
    fn is_second_law_violation(&self) -> bool {
        matches!(self, Self::SecondLawViolation)
    }
}

/// Outer model: given T8 (in kelvin), run the inner bisection on T9,
/// then compute HT recuperator forward UA.
struct OuterModel<'a, 'ctx, Fluid, Thermo> {
    ctx: &'a SolverContext<'ctx, Fluid, Thermo>,
}

impl<Fluid, Thermo> Model for OuterModel<'_, '_, Fluid, Thermo>
where
    Fluid: Clone,
    Thermo: ThermoModel<Fluid = Fluid>
        + HasPressure
        + HasEnthalpy
        + HasEntropy
        + StateFrom<(Fluid, ThermodynamicTemperature, Pressure)>
        + StateFrom<(Fluid, Pressure, SpecificEnthalpy)>
        + StateFrom<(Fluid, Pressure, SpecificEntropy)>,
{
    type Input = f64;
    type Output = OuterOutput<Fluid>;
    type Error = OuterError;

    fn call(&self, input: &f64) -> Result<OuterOutput<Fluid>, OuterError> {
        let t8_k = *input;
        let ctx = self.ctx;
        let t2_k = ctx.s2.temperature.get::<kelvin>();

        // Inner bisection on T9.
        //
        // Bracket: [T2, T8].
        // At T9 = T2: LT recup hot outlet equals cold inlet → max ΔT → UA_calc >> target → positive.
        // At T9 = T8: LT recup hot outlet equals hot inlet → zero heat transfer → UA_calc = 0 → negative.
        let inner_bracket = Bracket::new((t2_k, Sign::Positive), (t8_k, Sign::Negative))
            .map_err(|e| OuterError::Other(format!("inner bracket: {e}")))?;

        let inner_model = InnerModel { ctx, t8_k };
        let inner_problem = InnerProblem::<Fluid> {
            ua_target: ctx.lt_ua_target.get::<watt_per_kelvin>(),
            _fluid: PhantomData,
        };

        let inner_observer =
            |event: &Event<'_, InnerModel<'_, '_, Fluid, Thermo>, InnerProblem<Fluid>>| match event
            {
                Event::ModelFailed { error, .. } if error.is_second_law_violation() => {
                    Some(Action::assume_positive())
                }
                _ => None,
            };

        let inner_solution = bisection::solve_from_bracket(
            &inner_model,
            &inner_problem,
            inner_bracket,
            ctx.bisection_config,
            inner_observer,
        )
        .map_err(|e| OuterError::Other(format!("inner bisection: {e}")))?;

        let inner_output = inner_solution.snapshot.output;

        // HT recuperator forward solve.
        // Top = cold side (total flow: s4 → s5).
        // Bottom = hot side (total flow: s7 → s8).
        // Known outlet: bottom at T8.
        let ht_result = ctx
            .ht_recup
            .call(&RecuperatorGivenOutletInput {
                inlets: Inlets {
                    top: inner_output.s4.clone(),
                    bottom: ctx.s7.clone(),
                },
                mass_flows: MassFlows::new_unchecked(inner_output.m_dot_t, inner_output.m_dot_t),
                pressure_drops: ctx.ht_dp,
                outlet_temp: OutletTemp::Bottom(ThermodynamicTemperature::new::<kelvin>(t8_k)),
            })
            .map_err(|e| match &e {
                RecuperatorGivenOutletError::SecondLawViolation { .. } => {
                    OuterError::SecondLawViolation
                }
                _ => OuterError::Other(format!("HT recuperator: {e}")),
            })?;

        Ok(OuterOutput {
            ht_result,
            inner: inner_output,
        })
    }
}

/// Outer problem: residual = `UA_HT_calc` − `UA_HT_target`.
struct OuterProblem<Fluid> {
    ua_target: f64,
    _fluid: PhantomData<Fluid>,
}

impl<Fluid> EquationProblem<1> for OuterProblem<Fluid> {
    type Input = f64;
    type Output = OuterOutput<Fluid>;
    type Error = std::convert::Infallible;

    fn input(&self, x: &[f64; 1]) -> Result<f64, Self::Error> {
        Ok(x[0])
    }

    fn residuals(
        &self,
        _input: &f64,
        output: &OuterOutput<Fluid>,
    ) -> Result<[f64; 1], Self::Error> {
        let ua_calc = output.ht_result.ua.get::<watt_per_kelvin>();
        Ok([ua_calc - self.ua_target])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use twine_models::support::thermo::{fluid::CarbonDioxide, model::PerfectGas};
    use uom::si::{
        f64::{Power, Ratio, TemperatureInterval},
        power::megawatt,
        pressure::megapascal,
        ratio::ratio,
        temperature_interval::kelvin as kelvin_interval,
        thermal_conductance::kilowatt_per_kelvin,
        thermodynamic_temperature::degree_celsius,
    };

    use approx::assert_relative_eq;

    use crate::{
        config::{IsentropicEfficiency, PressureDrop, RecuperatorConfig},
        recompression::{HxConfig, TurboConfig},
    };

    // -- Helpers (PerfectGas, for input validation only) --

    fn pg_operating_point() -> OperatingPoint {
        OperatingPoint {
            t_comp_in: ThermodynamicTemperature::new::<degree_celsius>(50.0),
            t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(500.0),
            p_comp_in: Pressure::new::<megapascal>(0.1),
            p_comp_out: Pressure::new::<megapascal>(0.3),
            net_power: Power::new::<megawatt>(10.0),
        }
    }

    fn pg_config() -> Config {
        Config {
            turbo: TurboConfig {
                eta_mc: IsentropicEfficiency::new(0.89).unwrap(),
                eta_rc: IsentropicEfficiency::new(0.89).unwrap(),
                eta_turb: IsentropicEfficiency::new(0.93).unwrap(),
            },
            hx: HxConfig {
                lt_recuperator: RecuperatorConfig {
                    ua: ThermalConductance::new::<kilowatt_per_kelvin>(150.0),
                    segments: 10,
                    dp_cold: PressureDrop::None,
                    dp_hot: PressureDrop::None,
                },
                ht_recuperator: RecuperatorConfig {
                    ua: ThermalConductance::new::<kilowatt_per_kelvin>(150.0),
                    segments: 10,
                    dp_cold: PressureDrop::None,
                    dp_hot: PressureDrop::None,
                },
                precooler_dp: PressureDrop::None,
                primary_dp: PressureDrop::None,
            },
            temp_tol: TemperatureInterval::new::<kelvin_interval>(1e-6),
        }
    }

    fn solve_perfect_gas(
        op: OperatingPoint,
        recomp_frac: Ratio,
        config: &Config,
    ) -> Result<Solution<CarbonDioxide>, Error<CarbonDioxide>> {
        let thermo = PerfectGas::<CarbonDioxide>::new().unwrap();
        design_point(op, recomp_frac, config, CarbonDioxide, &thermo)
    }

    // -- Input validation tests (PerfectGas, no solver needed) --

    #[test]
    fn rejects_negative_recomp_frac() {
        let result = solve_perfect_gas(
            pg_operating_point(),
            Ratio::new::<ratio>(-0.1),
            &pg_config(),
        );
        assert!(matches!(result, Err(Error::InvalidRecompressionFraction)));
    }

    #[test]
    fn rejects_recomp_frac_at_one() {
        let result =
            solve_perfect_gas(pg_operating_point(), Ratio::new::<ratio>(1.0), &pg_config());
        assert!(matches!(result, Err(Error::InvalidRecompressionFraction)));
    }

    #[test]
    fn rejects_recomp_frac_above_one() {
        let result =
            solve_perfect_gas(pg_operating_point(), Ratio::new::<ratio>(1.5), &pg_config());
        assert!(matches!(result, Err(Error::InvalidRecompressionFraction)));
    }

    #[test]
    fn accepts_zero_recomp_frac() {
        // recomp_frac = 0 reduces to the simple cycle topology.
        // PerfectGas works fine here since there's no recompressor.
        let result =
            solve_perfect_gas(pg_operating_point(), Ratio::new::<ratio>(0.0), &pg_config());
        assert!(result.is_ok(), "expected Ok, got: {result:?}");
    }

    #[test]
    fn rejects_non_positive_net_power() {
        let mut op = pg_operating_point();
        op.net_power = Power::new::<megawatt>(0.0);
        let result = solve_perfect_gas(op, Ratio::new::<ratio>(0.3), &pg_config());
        assert!(matches!(result, Err(Error::NonPositiveNetPower)));
    }

    #[test]
    fn rejects_insufficient_pressure_rise() {
        let config = Config {
            hx: HxConfig {
                lt_recuperator: RecuperatorConfig {
                    dp_cold: PressureDrop::fraction(Ratio::new::<ratio>(0.4)).unwrap(),
                    dp_hot: PressureDrop::fraction(Ratio::new::<ratio>(0.4)).unwrap(),
                    ..pg_config().hx.lt_recuperator
                },
                ht_recuperator: RecuperatorConfig {
                    dp_cold: PressureDrop::fraction(Ratio::new::<ratio>(0.4)).unwrap(),
                    dp_hot: PressureDrop::fraction(Ratio::new::<ratio>(0.4)).unwrap(),
                    ..pg_config().hx.ht_recuperator
                },
                precooler_dp: PressureDrop::fraction(Ratio::new::<ratio>(0.4)).unwrap(),
                primary_dp: PressureDrop::fraction(Ratio::new::<ratio>(0.4)).unwrap(),
            },
            ..pg_config()
        };
        let result = solve_perfect_gas(pg_operating_point(), Ratio::new::<ratio>(0.3), &config);
        assert!(
            matches!(result, Err(Error::InsufficientPressureRise { .. })),
            "expected InsufficientPressureRise, got: {result:?}",
        );
    }

    // -- Pressure layout tests --

    #[test]
    fn cycle_pressures_no_drops() {
        let op = pg_operating_point();
        let config = pg_config();
        let pressures = CyclePressures::new::<CarbonDioxide>(&op, &config).unwrap();

        // With no pressure drops, high side is all P2 and low side is all P1.
        let p1 = op.p_comp_in.get::<megapascal>();
        let p2 = op.p_comp_out.get::<megapascal>();

        assert_relative_eq!(pressures.p1.get::<megapascal>(), p1);
        assert_relative_eq!(pressures.p2.get::<megapascal>(), p2);
        assert_relative_eq!(pressures.p3.get::<megapascal>(), p2);
        assert_relative_eq!(pressures.p4.get::<megapascal>(), p2);
        assert_relative_eq!(pressures.p5.get::<megapascal>(), p2);
        assert_relative_eq!(pressures.p6.get::<megapascal>(), p2);
        assert_relative_eq!(pressures.p7.get::<megapascal>(), p1);
        assert_relative_eq!(pressures.p8.get::<megapascal>(), p1);
        assert_relative_eq!(pressures.p9.get::<megapascal>(), p1);
        assert_relative_eq!(pressures.p10.get::<megapascal>(), p2);
    }

    #[test]
    fn mixing_valve_pressure_invariant() {
        let op = pg_operating_point();
        let config = Config {
            hx: HxConfig {
                lt_recuperator: RecuperatorConfig {
                    dp_cold: PressureDrop::fraction(Ratio::new::<ratio>(0.02)).unwrap(),
                    ..pg_config().hx.lt_recuperator
                },
                ..pg_config().hx
            },
            ..pg_config()
        };
        let pressures = CyclePressures::new::<CarbonDioxide>(&op, &config).unwrap();

        assert_relative_eq!(
            pressures.p4.get::<megapascal>(),
            pressures.p3.get::<megapascal>(),
        );
        assert_relative_eq!(
            pressures.p10.get::<megapascal>(),
            pressures.p3.get::<megapascal>(),
        );
    }

    // -- CoolProp sCO₂ tests --
    //
    // The recompression cycle relies on the density asymmetry near the
    // critical point — the main compressor does much less work than the
    // recompressor because the fluid is dense on the high-pressure side.
    // PerfectGas can't represent this, so all recomp_frac > 0 tests use
    // CoolProp at sCO₂ conditions.

    mod coolprop {
        use super::*;

        use approx::assert_relative_eq;
        use twine_models::support::thermo::model::CoolProp;
        use uom::si::{mass_rate::kilogram_per_second, power::kilowatt};

        fn sco2_operating_point() -> OperatingPoint {
            OperatingPoint {
                t_comp_in: ThermodynamicTemperature::new::<degree_celsius>(32.0),
                t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(550.0),
                p_comp_in: Pressure::new::<megapascal>(7.7),
                p_comp_out: Pressure::new::<megapascal>(20.0),
                net_power: Power::new::<megawatt>(10.0),
            }
        }

        fn sco2_config() -> Config {
            Config {
                turbo: TurboConfig {
                    eta_mc: IsentropicEfficiency::new(0.89).unwrap(),
                    eta_rc: IsentropicEfficiency::new(0.89).unwrap(),
                    eta_turb: IsentropicEfficiency::new(0.93).unwrap(),
                },
                hx: HxConfig {
                    lt_recuperator: RecuperatorConfig {
                        ua: ThermalConductance::new::<kilowatt_per_kelvin>(5000.0),
                        segments: 10,
                        dp_cold: PressureDrop::None,
                        dp_hot: PressureDrop::None,
                    },
                    ht_recuperator: RecuperatorConfig {
                        ua: ThermalConductance::new::<kilowatt_per_kelvin>(5000.0),
                        segments: 10,
                        dp_cold: PressureDrop::None,
                        dp_hot: PressureDrop::None,
                    },
                    precooler_dp: PressureDrop::None,
                    primary_dp: PressureDrop::None,
                },
                temp_tol: TemperatureInterval::new::<kelvin_interval>(1e-6),
            }
        }

        fn solve_sco2(
            op: OperatingPoint,
            recomp_frac: Ratio,
            config: &Config,
        ) -> Result<Solution<CarbonDioxide>, Error<CarbonDioxide>> {
            let thermo = CoolProp::<CarbonDioxide>::new().unwrap();
            design_point(op, recomp_frac, config, CarbonDioxide, &thermo)
        }

        #[test]
        fn solver_converges() {
            let result = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            );
            assert!(result.is_ok(), "expected Ok, got: {result:?}");
        }

        #[test]
        #[allow(clippy::similar_names)] // mc/rc are standard domain abbreviations.
        fn mass_balance() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            let m_dot_t = sol.m_dot_t.get::<kilogram_per_second>();
            let m_dot_mc = sol.m_dot_mc.get::<kilogram_per_second>();
            let m_dot_rc = sol.m_dot_rc.get::<kilogram_per_second>();

            assert_relative_eq!(m_dot_t, m_dot_mc + m_dot_rc, epsilon = 1e-10);
            assert!(m_dot_t > 0.0);
            assert!(m_dot_mc > 0.0);
            assert!(m_dot_rc > 0.0);
        }

        #[test]
        #[allow(clippy::similar_names)] // mc/rc are standard domain abbreviations.
        fn mass_flow_split_matches_recomp_frac() {
            let f = 0.3;
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(f),
                &sco2_config(),
            )
            .unwrap();

            let m_dot_t = sol.m_dot_t.get::<kilogram_per_second>();
            let m_dot_mc = sol.m_dot_mc.get::<kilogram_per_second>();
            let m_dot_rc = sol.m_dot_rc.get::<kilogram_per_second>();

            assert_relative_eq!(m_dot_mc, m_dot_t * (1.0 - f), epsilon = 1e-10);
            assert_relative_eq!(m_dot_rc, m_dot_t * f, epsilon = 1e-10);
        }

        #[test]
        fn energy_balance_closure() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            // First law: W_dot_net = Q_dot_phx - Q_dot_pc.
            let w_net = sol.w_dot_turb - sol.w_dot_mc - sol.w_dot_rc;
            let q_net = sol.q_dot_phx - sol.q_dot_pc;

            assert_relative_eq!(
                w_net.get::<kilowatt>(),
                q_net.get::<kilowatt>(),
                epsilon = 1.0, // 1 kW tolerance on a ~10 MW cycle.
            );
        }

        #[test]
        fn net_power_matches_target() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            let w_net =
                (sol.w_dot_turb - sol.w_dot_mc - sol.w_dot_rc).get::<uom::si::power::megawatt>();
            assert_relative_eq!(w_net, 10.0, epsilon = 1e-4);
        }

        #[test]
        fn thermal_efficiency_in_valid_range() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            let eta = sol.eta_thermal.get::<ratio>();
            assert!(eta > 0.0, "efficiency must be positive, got {eta}");
            assert!(eta < 1.0, "efficiency must be less than 1, got {eta}");
        }

        #[test]
        fn recuperator_min_delta_t_positive() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            let lt_dt = sol
                .lt_min_delta_t
                .get::<uom::si::temperature_interval::kelvin>();
            let ht_dt = sol
                .ht_min_delta_t
                .get::<uom::si::temperature_interval::kelvin>();

            assert!(lt_dt > 0.0, "LT min ΔT must be positive, got {lt_dt}");
            assert!(ht_dt > 0.0, "HT min ΔT must be positive, got {ht_dt}");
        }

        #[test]
        fn cold_path_temperatures_increase() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            let s = &sol.states;
            let temps: Vec<f64> = [&s.s1, &s.s2, &s.s3, &s.s4, &s.s5, &s.s6]
                .iter()
                .map(|st| st.temperature.get::<kelvin>())
                .collect();

            for i in 0..temps.len() - 1 {
                assert!(
                    temps[i] < temps[i + 1],
                    "cold path: T{} = {:.2} K ≥ T{} = {:.2} K",
                    i + 1,
                    temps[i],
                    i + 2,
                    temps[i + 1],
                );
            }
        }

        #[test]
        fn hot_path_temperatures_decrease() {
            let sol = solve_sco2(
                sco2_operating_point(),
                Ratio::new::<ratio>(0.3),
                &sco2_config(),
            )
            .unwrap();

            let s = &sol.states;
            let temps: Vec<f64> = [&s.s7, &s.s8, &s.s9]
                .iter()
                .map(|st| st.temperature.get::<kelvin>())
                .collect();

            for i in 0..temps.len() - 1 {
                assert!(
                    temps[i] > temps[i + 1],
                    "hot path: T{} = {:.2} K ≤ T{} = {:.2} K",
                    i + 7,
                    temps[i],
                    i + 8,
                    temps[i + 1],
                );
            }
        }
    }
}

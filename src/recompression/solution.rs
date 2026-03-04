use twine_models::{models::thermal::hx::discretized::Effectiveness, support::thermo::State};
use uom::si::f64::{MassRate, Power, Ratio, TemperatureInterval};

/// Model solution at the design point for a recompression Brayton cycle.
#[derive(Debug, Clone)]
pub struct Solution<Fluid> {
    /// Thermodynamic states around the cycle.
    pub states: CycleStates<Fluid>,

    /// Turbine mass flow rate (total).
    pub m_dot_t: MassRate,

    /// Main compressor mass flow rate.
    pub m_dot_mc: MassRate,

    /// Recompressor mass flow rate.
    pub m_dot_rc: MassRate,

    /// Main compressor power.
    pub w_dot_mc: Power,

    /// Recompressor power.
    pub w_dot_rc: Power,

    /// Turbine power.
    pub w_dot_turb: Power,

    /// Primary heat exchanger heat addition.
    pub q_dot_phx: Power,

    /// Precooler heat rejection.
    pub q_dot_pc: Power,

    /// Low-temperature recuperator heat transfer rate.
    pub q_dot_lt: Power,

    /// High-temperature recuperator heat transfer rate.
    pub q_dot_ht: Power,

    /// Thermal efficiency (`η = W_net / Q_in`).
    pub eta_thermal: Ratio,

    /// LT recuperator minimum hot-to-cold temperature difference.
    pub lt_min_delta_t: TemperatureInterval,

    /// HT recuperator minimum hot-to-cold temperature difference.
    pub ht_min_delta_t: TemperatureInterval,

    /// LT recuperator effectiveness.
    pub lt_effectiveness: Effectiveness,

    /// HT recuperator effectiveness.
    pub ht_effectiveness: Effectiveness,
}

/// Thermodynamic states around the recompression cycle.
///
/// State ordering (Fortran numbering):
///  1. Main compressor inlet / precooler outlet
///  2. Main compressor outlet / LT recuperator cold inlet
///  3. LT recuperator cold outlet / mixing valve inlet (main compressor side)
///  4. Mixing valve outlet / HT recuperator cold inlet
///  5. HT recuperator cold outlet / PHX inlet
///  6. PHX outlet / turbine inlet
///  7. Turbine outlet / HT recuperator hot inlet
///  8. HT recuperator hot outlet / LT recuperator hot inlet
///  9. LT recuperator hot outlet / precooler and recompressor inlet
/// 10. Recompressor outlet / mixing valve inlet (recompressor side)
#[derive(Debug, Clone)]
pub struct CycleStates<Fluid> {
    /// Main compressor inlet (precooler outlet).
    pub s1: State<Fluid>,

    /// Main compressor outlet (LT recuperator cold inlet).
    pub s2: State<Fluid>,

    /// LT recuperator cold outlet (mixing valve inlet, main compressor side).
    pub s3: State<Fluid>,

    /// Mixing valve outlet (HT recuperator cold inlet).
    pub s4: State<Fluid>,

    /// HT recuperator cold outlet (PHX inlet).
    pub s5: State<Fluid>,

    /// PHX outlet (turbine inlet).
    pub s6: State<Fluid>,

    /// Turbine outlet (HT recuperator hot inlet).
    pub s7: State<Fluid>,

    /// HT recuperator hot outlet (LT recuperator hot inlet).
    pub s8: State<Fluid>,

    /// LT recuperator hot outlet (precooler and recompressor inlet).
    pub s9: State<Fluid>,

    /// Recompressor outlet (mixing valve inlet, recompressor side).
    pub s10: State<Fluid>,
}

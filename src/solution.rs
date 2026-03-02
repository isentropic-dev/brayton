use twine_models::support::thermo::State;
use uom::si::f64::{MassRate, Power, Ratio};

/// Model solution at the design point for a closed Brayton cycle.
#[derive(Debug, Clone)]
pub struct Solution<Fluid> {
    /// Thermodynamic states around the cycle.
    pub states: CycleStates<Fluid>,

    /// Cycle mass flow rate.
    pub m_dot: MassRate,

    /// Compressor power.
    pub w_dot_comp: Power,

    /// Turbine power.
    pub w_dot_turb: Power,

    /// Precooler heat rejection.
    pub q_dot_pc: Power,

    /// Primary heat exchanger heat addition.
    pub q_dot_phx: Power,

    /// Thermal efficiency (`η = W_net / Q_in`).
    pub eta_thermal: Ratio,
}

/// Thermodynamic states around the cycle.
///
/// State ordering:
/// 1. Compressor inlet (precooler outlet)
/// 2. Compressor outlet
/// 3. Recuperator cold-side outlet (to primary heat exchanger)
/// 4. Primary heat exchanger outlet (turbine inlet)
/// 5. Turbine outlet (to recuperator hot side)
/// 6. Recuperator hot-side outlet (to precooler)
#[derive(Debug, Clone)]
pub struct CycleStates<Fluid> {
    /// Compressor inlet (precooler outlet).
    pub s1: State<Fluid>,

    /// Compressor outlet.
    pub s2: State<Fluid>,

    /// Recuperator cold-side outlet (to primary heat exchanger).
    pub s3: State<Fluid>,

    /// Primary heat exchanger outlet (turbine inlet).
    pub s4: State<Fluid>,

    /// Turbine outlet (to recuperator hot side).
    pub s5: State<Fluid>,

    /// Recuperator hot-side outlet (to precooler).
    pub s6: State<Fluid>,
}

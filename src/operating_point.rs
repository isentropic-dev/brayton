use twine_core::constraint::{Constrained, StrictlyPositive};
use uom::si::f64::{Power, Pressure, ThermodynamicTemperature};

/// Design-point operating conditions and targets for the simple Brayton cycle.
#[derive(Debug, Clone, Copy)]
pub struct OperatingPoint {
    /// Compressor inlet temperature (maintained by the precooler).
    pub t_comp_in: ThermodynamicTemperature,

    /// Turbine inlet temperature (maintained by the primary heat exchanger).
    pub t_turb_in: ThermodynamicTemperature,

    /// Compressor inlet pressure (minimum cycle pressure).
    pub p_comp_in: Pressure,

    /// Compressor outlet pressure (maximum cycle pressure).
    pub p_comp_out: Pressure,

    /// Net cycle power output (turbine power minus compressor power).
    pub net_power: Constrained<Power, StrictlyPositive>,
}

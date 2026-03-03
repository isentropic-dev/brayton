use twine_core::Model;
use twine_models::{
    models::thermal::hx::discretized::{
        Inlets, MassFlows, PressureDrops, Recuperator, RecuperatorConfig, RecuperatorInput,
    },
    support::{
        thermo::capability::{HasEnthalpy, HasEntropy, HasPressure, StateFrom, ThermoModel},
        turbomachinery::{compressor, turbine},
        units::{SpecificEnthalpy, SpecificEntropy},
    },
};
use uom::{
    ConstZero,
    si::f64::{Pressure, ThermodynamicTemperature},
};

use super::{Config, CycleStates, Error, OperatingPoint, Solution};

/// Solve for cycle performance at the design point.
///
/// Calculates all thermodynamic states, mass flow rate, component powers,
/// and thermal efficiency for a simple recuperated Brayton cycle given
/// operating conditions and hardware configuration.
///
/// # Errors
///
/// Returns an error on infeasible operating points, thermodynamic model
/// failures, or component model failures.
pub fn design_point<Fluid, Thermo>(
    operating_point: OperatingPoint,
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
    // Validate operating point.
    if operating_point.net_power.value <= 0.0 {
        return Err(Error::NonPositiveNetPower);
    }

    // Calculate pressures at all state points.
    // Work forward from the compressor inlet through the hot side,
    // and backward from the precooler outlet through the cold side.
    let p1 = operating_point.p_comp_in;
    let p2 = operating_point.p_comp_out;
    let p3 = config.hx.recuperator.dp_cold.outlet_pressure(p2);
    let p4 = config.hx.primary_dp.outlet_pressure(p3);
    let p6 = config.hx.precooler_dp.inlet_pressure(p1);
    let p5 = config.hx.recuperator.dp_hot.inlet_pressure(p6);

    // Validate pressures: turbine inlet must exceed turbine outlet.
    if p4 <= p5 {
        return Err(Error::InsufficientPressureRise {
            rise: p2 - p1,
            drop: (p2 - p4) + (p5 - p1),
        });
    }

    // Calculate known states from the operating point.
    let s1 = thermo
        .state_from((fluid.clone(), operating_point.t_comp_in, p1))
        .map_err(|e| Error::thermo_failed("state_from(compressor inlet)", e))?;
    let s4 = thermo
        .state_from((fluid.clone(), operating_point.t_turb_in, p4))
        .map_err(|e| Error::thermo_failed("state_from(turbine inlet)", e))?;

    // Go through compressor to define state 2.
    let compressor::CompressionResult {
        outlet: s2,
        work: comp_work,
    } = compressor::isentropic(&s1, p2, config.turbo.eta_comp, thermo)?;

    // Go through turbine to define state 5.
    let turbine::ExpansionResult {
        outlet: s5,
        work: turb_work,
    } = turbine::isentropic(&s4, p5, config.turbo.eta_turb, thermo)?;

    // Calculate net power and required mass flow rate.
    let w_net = turb_work.quantity() - comp_work.quantity();
    if w_net <= SpecificEnthalpy::ZERO {
        return Err(Error::InsufficientTurbineWork { w_net });
    }
    let m_dot = operating_point.net_power / w_net;

    // Solve recuperator with given UA to define states 3 and 6.
    let recuperator = Recuperator::new(
        thermo,
        config.hx.recuperator.segments,
        RecuperatorConfig::default(),
    )
    .map_err(Error::Recuperator)?;
    let recup_result = recuperator.call(&RecuperatorInput {
        inlets: Inlets {
            top: s2.clone(),
            bottom: s5.clone(),
        },
        mass_flows: MassFlows::new_unchecked(m_dot, m_dot),
        // top = cold side, bottom = hot side
        pressure_drops: PressureDrops::new_unchecked(p2 - p3, p5 - p6),
        ua: config.hx.recuperator.ua,
    })?;
    let s3 = recup_result.top_outlet;
    let s6 = recup_result.bottom_outlet;

    // Heat rejection from precooler energy balance: q = h6 - h1.
    let h6 = thermo
        .enthalpy(&s6)
        .map_err(|e| Error::thermo_failed("enthalpy(precooler inlet)", e))?;
    let h1 = thermo
        .enthalpy(&s1)
        .map_err(|e| Error::thermo_failed("enthalpy(precooler outlet)", e))?;
    let q_pc = h6 - h1;

    // Heat addition from PHX energy balance: q = h4 - h3.
    let h4 = thermo
        .enthalpy(&s4)
        .map_err(|e| Error::thermo_failed("enthalpy(phx outlet)", e))?;
    let h3 = thermo
        .enthalpy(&s3)
        .map_err(|e| Error::thermo_failed("enthalpy(phx inlet)", e))?;
    let q_phx = h4 - h3;

    // Thermal efficiency: net work output / heat input.
    let eta_thermal = w_net / q_phx;

    // Convert specific quantities to power and heat rates.
    let w_dot_comp = comp_work.quantity() * m_dot;
    let w_dot_turb = turb_work.quantity() * m_dot;
    let q_dot_phx = q_phx * m_dot;
    let q_dot_pc = q_pc * m_dot;

    Ok(Solution {
        states: CycleStates {
            s1,
            s2,
            s3,
            s4,
            s5,
            s6,
        },
        m_dot,
        w_dot_comp,
        w_dot_turb,
        q_dot_pc,
        q_dot_phx,
        eta_thermal,
    })
}

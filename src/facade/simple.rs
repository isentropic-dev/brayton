use twine_models::{
    models::thermal::hx::discretized::{Inlets, MassFlows, PressureDrops},
    support::thermo::{
        capability::{HasEnthalpy, HasEntropy, HasPressure},
        fluid::CarbonDioxide,
        model::{CoolProp, PerfectGas},
    },
};
use uom::si::{
    f64::{Power, Pressure, Ratio, ThermalConductance, ThermodynamicTemperature},
    mass_rate::kilogram_per_second,
    power::megawatt,
    pressure::megapascal,
    ratio::ratio,
    temperature_interval::kelvin as delta_kelvin,
    thermal_conductance::kilowatt_per_kelvin,
    thermodynamic_temperature::degree_celsius,
};

use super::StatePoint;
use crate::{
    IsentropicEfficiency, OperatingPoint, PressureDrop, RecuperatorConfig,
    effectiveness::{self, EffectivenessThermo},
    fluids::{Butane, Helium, Nitrogen},
    simple::{Config, HxConfig, TurboConfig},
};

/// Input for a simple recuperated Brayton cycle design-point calculation.
///
/// All temperatures in °C, pressures in kPa, power in kW,
/// thermal conductance in kW/K, efficiencies and pressure drop
/// fractions as dimensionless ratios (0–1).
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct DesignPointInput {
    /// Thermodynamic model to use.
    ///
    /// `"PerfectGas"` (default) or `"CoolProp"`.
    #[cfg_attr(feature = "wasm", serde(default = "default_model"))]
    pub model: String,

    /// Working fluid for `CoolProp` calculations.
    ///
    /// `"CarbonDioxide"` (default), `"Nitrogen"`, `"Helium"`, or `"Butane"`.
    /// Ignored when `model` is `"PerfectGas"` (always uses CO₂).
    #[cfg_attr(feature = "wasm", serde(default = "super::default_fluid"))]
    pub fluid: String,

    // Operating point
    /// Compressor inlet temperature in degrees Celsius.
    pub compressor_inlet_temp_c: f64,

    /// Turbine inlet temperature in degrees Celsius.
    pub turbine_inlet_temp_c: f64,

    /// Compressor inlet (minimum cycle) pressure in megapascals.
    pub compressor_inlet_pressure_mpa: f64,

    /// Compressor outlet (maximum cycle) pressure in megapascals.
    pub compressor_outlet_pressure_mpa: f64,

    /// Target net cycle power output in megawatts.
    pub net_power_mw: f64,

    // Turbomachinery
    /// Compressor isentropic efficiency as a dimensionless ratio (0–1).
    pub compressor_efficiency: f64,

    /// Turbine isentropic efficiency as a dimensionless ratio (0–1).
    pub turbine_efficiency: f64,

    // Recuperator
    /// Recuperator overall thermal conductance (UA) in kilowatts per kelvin.
    pub recuperator_ua_kw_per_k: f64,

    /// Number of discretization segments for the recuperator model.
    ///
    /// Supported values: 1, 5, 10, 20, 50.
    pub recuperator_segments: usize,

    /// Recuperator cold-side fractional pressure drop (0–1).
    pub recuperator_dp_cold_fraction: f64,

    /// Recuperator hot-side fractional pressure drop (0–1).
    pub recuperator_dp_hot_fraction: f64,

    // Other HX pressure drops
    /// Precooler fractional pressure drop (0–1).
    pub precooler_dp_fraction: f64,

    /// Primary heat exchanger fractional pressure drop (0–1).
    pub primary_hx_dp_fraction: f64,
}

/// Thermodynamic performance output for a design-point calculation.
#[derive(Debug)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct DesignPointOutput {
    /// Cycle mass flow rate in kilograms per second.
    pub mass_flow_kg_per_s: f64,

    /// Compressor power consumption in megawatts.
    pub compressor_power_mw: f64,

    /// Turbine power output in megawatts.
    pub turbine_power_mw: f64,

    /// Net cycle power output (turbine minus compressor) in megawatts.
    pub net_power_mw: f64,

    /// Primary heat exchanger heat addition rate in megawatts.
    pub heat_input_mw: f64,

    /// Precooler heat rejection rate in megawatts.
    pub heat_rejection_mw: f64,

    /// Cycle thermal efficiency (`η = W_net / Q_in`), dimensionless.
    pub thermal_efficiency: f64,

    /// Recuperator heat transfer rate in megawatts.
    pub recuperator_heat_transfer_mw: f64,

    /// Minimum hot-to-cold temperature difference in the recuperator, in kelvin.
    pub recuperator_min_delta_t_k: f64,

    /// Recuperator effectiveness (dimensionless, 0 to 1).
    ///
    /// `None` if the pinch-point calculation did not converge.
    pub recuperator_effectiveness: Option<f64>,

    /// Thermodynamic states at the six cycle points.
    ///
    /// Index order:
    /// - 0 — compressor inlet (precooler outlet)
    /// - 1 — compressor outlet
    /// - 2 — recuperator cold-side outlet (to primary HX)
    /// - 3 — primary HX outlet (turbine inlet)
    /// - 4 — turbine outlet (to recuperator hot side)
    /// - 5 — recuperator hot-side outlet (to precooler)
    pub states: [StatePoint; 6],
}

/// Default model name for serde deserialization.
#[cfg(feature = "wasm")]
fn default_model() -> String {
    String::from("PerfectGas")
}

/// Run a simple recuperated Brayton cycle design-point calculation.
///
/// Dispatches to the thermodynamic model specified in `input.model`
/// (`"PerfectGas"` or `"CoolProp"`) and, for `CoolProp`, the fluid
/// specified in `input.fluid`.
///
/// # Errors
///
/// Returns a descriptive error string on invalid input or solver failure.
pub fn design_point(input: &DesignPointInput) -> Result<DesignPointOutput, String> {
    /// Construct a `CoolProp` model, solve, and convert to output.
    fn solve_coolprop<
        F: Clone + Default + twine_models::support::thermo::model::coolprop::CoolPropFluid,
    >(
        op: crate::OperatingPoint,
        config: &Config,
    ) -> Result<DesignPointOutput, String> {
        let thermo = CoolProp::<F>::new()
            .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
        let solution = crate::simple::cycle::design_point(op, config, F::default(), &thermo)
            .map_err(|e| e.to_string())?;
        Ok(convert_output(&solution, config, &thermo))
    }

    let (op, config) = convert_input(input)?;

    match input.model.as_str() {
        "PerfectGas" => {
            let thermo = PerfectGas::<CarbonDioxide>::new()
                .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
            let solution = crate::simple::cycle::design_point(op, &config, CarbonDioxide, &thermo)
                .map_err(|e| e.to_string())?;
            Ok(convert_output(&solution, &config, &thermo))
        }
        "CoolProp" => match input.fluid.as_str() {
            "CarbonDioxide" => solve_coolprop::<CarbonDioxide>(op, &config),
            "Nitrogen" => solve_coolprop::<Nitrogen>(op, &config),
            "Helium" => solve_coolprop::<Helium>(op, &config),
            "Butane" => solve_coolprop::<Butane>(op, &config),
            other => Err(format!("unknown fluid: {other}")),
        },
        other => Err(format!("unknown model: {other}")),
    }
}

/// Convert a [`DesignPointInput`] into core types, validating all fields.
fn convert_input(input: &DesignPointInput) -> Result<(OperatingPoint, Config), String> {
    let op = OperatingPoint {
        t_comp_in: ThermodynamicTemperature::new::<degree_celsius>(input.compressor_inlet_temp_c),
        t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(input.turbine_inlet_temp_c),
        p_comp_in: Pressure::new::<megapascal>(input.compressor_inlet_pressure_mpa),
        p_comp_out: Pressure::new::<megapascal>(input.compressor_outlet_pressure_mpa),
        net_power: Power::new::<megawatt>(input.net_power_mw),
    };

    let eta_comp = IsentropicEfficiency::new(input.compressor_efficiency)
        .map_err(|e| format!("invalid compressor_efficiency: {e}"))?;
    let eta_turb = IsentropicEfficiency::new(input.turbine_efficiency)
        .map_err(|e| format!("invalid turbine_efficiency: {e}"))?;

    let dp_recup_cold =
        PressureDrop::fraction(Ratio::new::<ratio>(input.recuperator_dp_cold_fraction))
            .map_err(|e| format!("invalid recuperator_dp_cold_fraction: {e}"))?;
    let dp_recup_hot =
        PressureDrop::fraction(Ratio::new::<ratio>(input.recuperator_dp_hot_fraction))
            .map_err(|e| format!("invalid recuperator_dp_hot_fraction: {e}"))?;
    let dp_precooler = PressureDrop::fraction(Ratio::new::<ratio>(input.precooler_dp_fraction))
        .map_err(|e| format!("invalid precooler_dp_fraction: {e}"))?;
    let dp_primary = PressureDrop::fraction(Ratio::new::<ratio>(input.primary_hx_dp_fraction))
        .map_err(|e| format!("invalid primary_hx_dp_fraction: {e}"))?;

    let config = Config {
        turbo: TurboConfig { eta_comp, eta_turb },
        hx: HxConfig {
            recuperator: RecuperatorConfig {
                ua: ThermalConductance::new::<kilowatt_per_kelvin>(input.recuperator_ua_kw_per_k),
                segments: input.recuperator_segments,
                dp_cold: dp_recup_cold,
                dp_hot: dp_recup_hot,
            },
            precooler_dp: dp_precooler,
            primary_dp: dp_primary,
        },
    };

    Ok((op, config))
}

/// Convert a [`crate::simple::Solution`] to a [`DesignPointOutput`], extracting plain-data values.
fn convert_output<Fluid, Thermo>(
    solution: &crate::simple::Solution<Fluid>,
    config: &Config,
    thermo: &Thermo,
) -> DesignPointOutput
where
    Fluid: Clone,
    Thermo: HasPressure<Fluid = Fluid>
        + HasEnthalpy<Fluid = Fluid>
        + HasEntropy<Fluid = Fluid>
        + EffectivenessThermo<Fluid>,
{
    let states = &solution.states;
    let state_array = [
        &states.s1, &states.s2, &states.s3, &states.s4, &states.s5, &states.s6,
    ];

    let points = state_array.map(|s| crate::thermo::state_to_point(s, thermo));

    let w_dot_net = solution.w_dot_turb - solution.w_dot_comp;

    // Recuperator: top = cold (s2→s3), bottom = hot (s5→s6).
    let p2 = thermo.pressure(&states.s2).expect("p2");
    let p3 = thermo.pressure(&states.s3).expect("p3");
    let p5 = thermo.pressure(&states.s5).expect("p5");
    let p6 = thermo.pressure(&states.s6).expect("p6");

    let effectiveness = effectiveness::compute(
        Inlets {
            top: states.s2.clone(),
            bottom: states.s5.clone(),
        },
        MassFlows::new_unchecked(solution.m_dot, solution.m_dot),
        PressureDrops::new_unchecked(p2 - p3, p5 - p6),
        solution.q_dot_recup,
        config.hx.recuperator.segments,
        thermo,
    );

    DesignPointOutput {
        mass_flow_kg_per_s: solution.m_dot.get::<kilogram_per_second>(),
        compressor_power_mw: solution.w_dot_comp.get::<megawatt>(),
        turbine_power_mw: solution.w_dot_turb.get::<megawatt>(),
        net_power_mw: w_dot_net.get::<megawatt>(),
        heat_input_mw: solution.q_dot_phx.get::<megawatt>(),
        heat_rejection_mw: solution.q_dot_pc.get::<megawatt>(),
        thermal_efficiency: solution.eta_thermal.get::<ratio>(),
        recuperator_heat_transfer_mw: solution.q_dot_recup.get::<megawatt>(),
        recuperator_min_delta_t_k: solution.recuperator_min_delta_t.get::<delta_kelvin>(),
        recuperator_effectiveness: effectiveness,
        states: points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_input() -> DesignPointInput {
        DesignPointInput {
            model: String::from("PerfectGas"),
            fluid: String::from("CarbonDioxide"),
            compressor_inlet_temp_c: 50.0,
            turbine_inlet_temp_c: 500.0,
            compressor_inlet_pressure_mpa: 0.1,
            compressor_outlet_pressure_mpa: 0.3,
            net_power_mw: 10.0,
            compressor_efficiency: 0.89,
            turbine_efficiency: 0.93,
            recuperator_ua_kw_per_k: 2000.0,
            recuperator_segments: 10,
            recuperator_dp_cold_fraction: 0.02,
            recuperator_dp_hot_fraction: 0.02,
            precooler_dp_fraction: 0.01,
            primary_hx_dp_fraction: 0.01,
        }
    }

    #[test]
    fn smoke_test_baseline() {
        let result = design_point(&baseline_input());
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let out = result.unwrap();

        // Physical constraints.
        assert!(out.mass_flow_kg_per_s > 0.0);
        assert!(out.turbine_power_mw > out.compressor_power_mw);
        assert!(out.thermal_efficiency > 0.0);
        assert!(out.thermal_efficiency < 1.0);
        assert!(out.heat_input_mw > 0.0);
        assert!(out.heat_rejection_mw > 0.0);

        // Net power close to target.
        let relative_error = (out.net_power_mw - 10.0).abs() / 10.0;
        assert!(
            relative_error < 1e-6,
            "net power {:.6} MW deviates too far from target",
            out.net_power_mw,
        );

        // Effectiveness must be physical.
        let eff = out
            .recuperator_effectiveness
            .expect("effectiveness should converge for baseline");
        assert!(
            (0.0..=1.0).contains(&eff),
            "effectiveness {eff} is outside [0, 1]",
        );
    }

    #[test]
    fn all_state_points_are_valid() {
        let out = design_point(&baseline_input()).unwrap();

        for (i, state) in out.states.iter().enumerate() {
            assert!(
                !state.temperature_c.is_nan(),
                "state {i} temperature is NaN",
            );
            assert!(
                !state.pressure_mpa.is_nan() && state.pressure_mpa > 0.0,
                "state {i} pressure is invalid: {}",
                state.pressure_mpa,
            );
            assert!(
                !state.density_kg_per_m3.is_nan() && state.density_kg_per_m3 > 0.0,
                "state {i} density is invalid: {}",
                state.density_kg_per_m3,
            );
            assert!(
                !state.enthalpy_kj_per_kg.is_nan(),
                "state {i} enthalpy is NaN",
            );
            assert!(
                !state.entropy_kj_per_kg_k.is_nan(),
                "state {i} entropy is NaN",
            );
        }
    }

    /// Pressures must decrease monotonically along each flow path:
    /// high side (P2 > P3 > P4) and low side (P5 > P6 > P1).
    #[test]
    fn pressures_decrease_along_flow_path() {
        let out = design_point(&baseline_input()).unwrap();
        let [s1, s2, s3, s4, s5, s6] = &out.states;

        // High-pressure side: compressor outlet → recuperator cold → PHX → turbine inlet.
        assert!(
            s2.pressure_mpa > s3.pressure_mpa,
            "P2 ({}) must exceed P3 ({})",
            s2.pressure_mpa,
            s3.pressure_mpa,
        );
        assert!(
            s3.pressure_mpa > s4.pressure_mpa,
            "P3 ({}) must exceed P4 ({})",
            s3.pressure_mpa,
            s4.pressure_mpa,
        );

        // Low-pressure side: turbine outlet → recuperator hot → precooler → compressor inlet.
        assert!(
            s5.pressure_mpa > s6.pressure_mpa,
            "P5 ({}) must exceed P6 ({})",
            s5.pressure_mpa,
            s6.pressure_mpa,
        );
        assert!(
            s6.pressure_mpa > s1.pressure_mpa,
            "P6 ({}) must exceed P1 ({})",
            s6.pressure_mpa,
            s1.pressure_mpa,
        );
    }

    #[test]
    fn smoke_test_coolprop() {
        // Dashboard-default sCO₂ conditions.
        let input = DesignPointInput {
            model: String::from("CoolProp"),
            compressor_inlet_temp_c: 32.0,
            compressor_inlet_pressure_mpa: 8.0,
            compressor_outlet_pressure_mpa: 20.0,
            turbine_inlet_temp_c: 550.0,
            ..baseline_input()
        };
        let result = design_point(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let out = result.unwrap();

        assert!(out.mass_flow_kg_per_s > 0.0);
        assert!(out.turbine_power_mw > out.compressor_power_mw);
        assert!(out.thermal_efficiency > 0.0);
        assert!(out.thermal_efficiency < 1.0);
    }

    #[test]
    fn smoke_test_coolprop_nitrogen() {
        // Verify non-CO₂ fluid dispatch works end-to-end.
        // One non-CO₂ fluid is sufficient — the others follow the same
        // code path and differ only in the CoolPropFluid::NAME constant.
        let input = DesignPointInput {
            model: String::from("CoolProp"),
            fluid: String::from("Nitrogen"),
            compressor_inlet_temp_c: 30.0,
            compressor_inlet_pressure_mpa: 0.1,
            compressor_outlet_pressure_mpa: 0.3,
            turbine_inlet_temp_c: 500.0,
            recuperator_segments: 10,
            ..baseline_input()
        };
        let result = design_point(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let out = result.unwrap();

        assert!(out.thermal_efficiency > 0.0);
        assert!(out.thermal_efficiency < 1.0);
    }

    #[test]
    fn unknown_fluid_returns_error() {
        let input = DesignPointInput {
            model: String::from("CoolProp"),
            fluid: String::from("Unobtanium"),
            ..baseline_input()
        };
        let result = design_point(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown fluid"));
    }

    #[test]
    fn unknown_model_returns_error() {
        let input = DesignPointInput {
            model: String::from("Bogus"),
            ..baseline_input()
        };
        let result = design_point(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown model"));
    }

    #[test]
    fn invalid_compressor_efficiency_returns_error() {
        let input = DesignPointInput {
            compressor_efficiency: 1.5,
            ..baseline_input()
        };
        let result = design_point(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("compressor_efficiency"));
    }

    /// Dashboard defaults: effectiveness must be consistent with min ΔT.
    ///
    /// If min ΔT is well above zero, the recuperator isn't at its
    /// thermodynamic limit and effectiveness must be below 1.0.
    #[test]
    fn effectiveness_consistent_with_min_delta_t() {
        for model in ["PerfectGas", "CoolProp"] {
            let input = DesignPointInput {
                model: String::from(model),
                compressor_inlet_temp_c: 32.0,
                compressor_inlet_pressure_mpa: 8.0,
                compressor_outlet_pressure_mpa: 20.0,
                turbine_inlet_temp_c: 550.0,
                recuperator_ua_kw_per_k: 1000.0,
                ..baseline_input()
            };
            let out = design_point(&input).unwrap();

            let eff = out
                .recuperator_effectiveness
                .expect("effectiveness should converge");

            // If min ΔT is significantly above zero, effectiveness must be below 1.0.
            assert!(
                !(out.recuperator_min_delta_t_k > 1.0 && eff >= 1.0),
                "[{model}] effectiveness {eff} should be < 1.0 \
                 when min ΔT is {:.1} K",
                out.recuperator_min_delta_t_k,
            );
        }
    }

    #[test]
    fn invalid_pressure_drop_fraction_returns_error() {
        let input = DesignPointInput {
            recuperator_dp_cold_fraction: -0.01,
            ..baseline_input()
        };
        let result = design_point(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("recuperator_dp_cold_fraction"));
    }
}

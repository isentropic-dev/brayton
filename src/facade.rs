use twine_models::support::thermo::{
    capability::{HasEnthalpy, HasEntropy, HasPressure},
    fluid::CarbonDioxide,
    model::PerfectGas,
};
use uom::si::{
    available_energy::kilojoule_per_kilogram,
    f64::{Power, Pressure, Ratio, ThermalConductance, ThermodynamicTemperature},
    mass_density::kilogram_per_cubic_meter,
    mass_rate::kilogram_per_second,
    power::kilowatt,
    pressure::kilopascal,
    ratio::ratio,
    specific_heat_capacity::kilojoule_per_kilogram_kelvin,
    thermal_conductance::kilowatt_per_kelvin,
    thermodynamic_temperature::degree_celsius,
};

use crate::{
    Config, HxConfig, IsentropicEfficiency, OperatingPoint, PressureDrop, RecuperatorConfig,
    TurboConfig,
};

/// Input for a simple recuperated Brayton cycle design-point calculation.
///
/// All temperatures in °C, pressures in kPa, power in kW,
/// thermal conductance in kW/K, efficiencies and pressure drop
/// fractions as dimensionless ratios (0–1).
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct DesignPointInput {
    // Operating point
    /// Compressor inlet temperature in degrees Celsius.
    pub compressor_inlet_temp_c: f64,

    /// Turbine inlet temperature in degrees Celsius.
    pub turbine_inlet_temp_c: f64,

    /// Compressor inlet (minimum cycle) pressure in kilopascals.
    pub compressor_inlet_pressure_kpa: f64,

    /// Compressor outlet (maximum cycle) pressure in kilopascals.
    pub compressor_outlet_pressure_kpa: f64,

    /// Target net cycle power output in kilowatts.
    pub net_power_kw: f64,

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

    /// Compressor power consumption in kilowatts.
    pub compressor_power_kw: f64,

    /// Turbine power output in kilowatts.
    pub turbine_power_kw: f64,

    /// Net cycle power output (turbine minus compressor) in kilowatts.
    pub net_power_kw: f64,

    /// Primary heat exchanger heat addition rate in kilowatts.
    pub heat_input_kw: f64,

    /// Precooler heat rejection rate in kilowatts.
    pub heat_rejection_kw: f64,

    /// Cycle thermal efficiency (`η = W_net / Q_in`), dimensionless.
    pub thermal_efficiency: f64,

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

/// Thermodynamic state at a single cycle point.
#[derive(Debug)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct StatePoint {
    /// Temperature in degrees Celsius.
    pub temperature_c: f64,

    /// Pressure in kilopascals.
    pub pressure_kpa: f64,

    /// Mass density in kilograms per cubic metre.
    pub density_kg_per_m3: f64,

    /// Specific enthalpy in kilojoules per kilogram.
    pub enthalpy_kj_per_kg: f64,

    /// Specific entropy in kilojoules per kilogram-kelvin.
    pub entropy_kj_per_kg_k: f64,
}

/// Run a simple recuperated Brayton cycle design-point calculation.
///
/// Hardcodes [`PerfectGas`]`<`[`CarbonDioxide`]`>` as the thermodynamic model.
/// Takes plain-data input and returns plain-data output, suitable for FFI,
/// WASM, and Python bindings.
///
/// # Errors
///
/// Returns a descriptive error string on invalid input or solver failure.
pub fn design_point(input: &DesignPointInput) -> Result<DesignPointOutput, String> {
    let (op, config) = convert_input(input)?;
    let thermo = PerfectGas::<CarbonDioxide>::new()
        .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
    let solution = crate::cycle::design_point(op, &config, CarbonDioxide, &thermo)
        .map_err(|e| e.to_string())?;
    Ok(convert_output(&solution, &thermo))
}

/// Convert a [`DesignPointInput`] into core types, validating all fields.
fn convert_input(input: &DesignPointInput) -> Result<(OperatingPoint, Config), String> {
    let op = OperatingPoint {
        t_comp_in: ThermodynamicTemperature::new::<degree_celsius>(input.compressor_inlet_temp_c),
        t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(input.turbine_inlet_temp_c),
        p_comp_in: Pressure::new::<kilopascal>(input.compressor_inlet_pressure_kpa),
        p_comp_out: Pressure::new::<kilopascal>(input.compressor_outlet_pressure_kpa),
        net_power: Power::new::<kilowatt>(input.net_power_kw),
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

/// Convert a [`crate::Solution`] to a [`DesignPointOutput`], extracting plain-data values.
fn convert_output(
    solution: &crate::Solution<CarbonDioxide>,
    thermo: &PerfectGas<CarbonDioxide>,
) -> DesignPointOutput {
    let states = &solution.states;
    let state_array = [
        &states.s1, &states.s2, &states.s3, &states.s4, &states.s5, &states.s6,
    ];

    let points = state_array.map(|s| {
        let pressure = thermo.pressure(s).expect("pressure must be defined");
        let enthalpy = thermo.enthalpy(s).expect("enthalpy must be defined");
        let entropy = thermo.entropy(s).expect("entropy must be defined");

        StatePoint {
            temperature_c: s.temperature.get::<degree_celsius>(),
            pressure_kpa: pressure.get::<kilopascal>(),
            density_kg_per_m3: s.density.get::<kilogram_per_cubic_meter>(),
            enthalpy_kj_per_kg: enthalpy.get::<kilojoule_per_kilogram>(),
            entropy_kj_per_kg_k: entropy.get::<kilojoule_per_kilogram_kelvin>(),
        }
    });

    let w_dot_net = solution.w_dot_turb - solution.w_dot_comp;

    DesignPointOutput {
        mass_flow_kg_per_s: solution.m_dot.get::<kilogram_per_second>(),
        compressor_power_kw: solution.w_dot_comp.get::<kilowatt>(),
        turbine_power_kw: solution.w_dot_turb.get::<kilowatt>(),
        net_power_kw: w_dot_net.get::<kilowatt>(),
        heat_input_kw: solution.q_dot_phx.get::<kilowatt>(),
        heat_rejection_kw: solution.q_dot_pc.get::<kilowatt>(),
        thermal_efficiency: solution.eta_thermal.get::<ratio>(),
        states: points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_input() -> DesignPointInput {
        DesignPointInput {
            compressor_inlet_temp_c: 50.0,
            turbine_inlet_temp_c: 500.0,
            compressor_inlet_pressure_kpa: 100.0,
            compressor_outlet_pressure_kpa: 300.0,
            net_power_kw: 10_000.0,
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
        assert!(out.turbine_power_kw > out.compressor_power_kw);
        assert!(out.thermal_efficiency > 0.0);
        assert!(out.thermal_efficiency < 1.0);
        assert!(out.heat_input_kw > 0.0);
        assert!(out.heat_rejection_kw > 0.0);

        // Net power close to target.
        let relative_error = (out.net_power_kw - 10_000.0).abs() / 10_000.0;
        assert!(
            relative_error < 1e-6,
            "net power {:.3} kW deviates too far from target",
            out.net_power_kw,
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
                !state.pressure_kpa.is_nan() && state.pressure_kpa > 0.0,
                "state {i} pressure is invalid: {}",
                state.pressure_kpa,
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
            s2.pressure_kpa > s3.pressure_kpa,
            "P2 ({}) must exceed P3 ({})",
            s2.pressure_kpa,
            s3.pressure_kpa,
        );
        assert!(
            s3.pressure_kpa > s4.pressure_kpa,
            "P3 ({}) must exceed P4 ({})",
            s3.pressure_kpa,
            s4.pressure_kpa,
        );

        // Low-pressure side: turbine outlet → recuperator hot → precooler → compressor inlet.
        assert!(
            s5.pressure_kpa > s6.pressure_kpa,
            "P5 ({}) must exceed P6 ({})",
            s5.pressure_kpa,
            s6.pressure_kpa,
        );
        assert!(
            s6.pressure_kpa > s1.pressure_kpa,
            "P6 ({}) must exceed P1 ({})",
            s6.pressure_kpa,
            s1.pressure_kpa,
        );
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

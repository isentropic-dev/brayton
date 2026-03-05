//! WASM-facing facade for the recompression Brayton cycle.

use twine_models::{
    models::thermal::hx::discretized::{Inlets, MassFlows, PressureDrops},
    support::thermo::capability::{HasEnthalpy, HasEntropy, HasPressure},
};
use uom::si::{
    f64::{
        Power, Pressure, Ratio, TemperatureInterval, ThermalConductance, ThermodynamicTemperature,
    },
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
    recompression::{self, Config, HxConfig, TurboConfig},
};

/// Input for a recompression Brayton cycle design-point calculation.
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct RecompDesignPointInput {
    /// Thermodynamic model: `"CoolProp"` (default).
    ///
    /// `"PerfectGas"` is accepted but will fail for `recomp_frac > 0`
    /// because ideal gas lacks the density asymmetry the cycle requires.
    #[cfg_attr(feature = "wasm", serde(default = "default_model"))]
    pub model: String,

    /// Working fluid (default `"CarbonDioxide"`).
    #[cfg_attr(feature = "wasm", serde(default = "super::default_fluid"))]
    pub fluid: String,

    // Operating point.
    /// Compressor inlet temperature in degrees Celsius.
    pub compressor_inlet_temp_c: f64,
    /// Turbine inlet temperature in degrees Celsius.
    pub turbine_inlet_temp_c: f64,
    /// Compressor inlet pressure in megapascals.
    pub compressor_inlet_pressure_mpa: f64,
    /// Compressor outlet pressure in megapascals.
    pub compressor_outlet_pressure_mpa: f64,
    /// Target net cycle power output in megawatts.
    pub net_power_mw: f64,

    /// Recompression fraction (0–1, exclusive of 1).
    pub recomp_frac: f64,

    // Turbomachinery.
    /// Main compressor isentropic efficiency (0–1).
    pub mc_efficiency: f64,
    /// Recompressor isentropic efficiency (0–1).
    pub rc_efficiency: f64,
    /// Turbine isentropic efficiency (0–1).
    pub turbine_efficiency: f64,

    // LT recuperator.
    /// LT recuperator UA in kW/K.
    pub lt_recuperator_ua_kw_per_k: f64,
    /// LT recuperator discretization segments.
    pub lt_recuperator_segments: usize,
    /// LT recuperator cold-side fractional pressure drop (0–1).
    pub lt_recuperator_dp_cold_fraction: f64,
    /// LT recuperator hot-side fractional pressure drop (0–1).
    pub lt_recuperator_dp_hot_fraction: f64,

    // HT recuperator.
    /// HT recuperator UA in kW/K.
    pub ht_recuperator_ua_kw_per_k: f64,
    /// HT recuperator discretization segments.
    pub ht_recuperator_segments: usize,
    /// HT recuperator cold-side fractional pressure drop (0–1).
    pub ht_recuperator_dp_cold_fraction: f64,
    /// HT recuperator hot-side fractional pressure drop (0–1).
    pub ht_recuperator_dp_hot_fraction: f64,

    // Other HX pressure drops.
    /// Precooler fractional pressure drop (0–1).
    pub precooler_dp_fraction: f64,
    /// Primary HX fractional pressure drop (0–1).
    pub primary_hx_dp_fraction: f64,
}

/// Output for a recompression Brayton cycle design-point calculation.
#[derive(Debug)]
#[cfg_attr(feature = "wasm", derive(serde::Serialize, serde::Deserialize))]
pub struct RecompDesignPointOutput {
    /// Total mass flow rate (kg/s).
    pub mass_flow_total_kg_per_s: f64,
    /// Main compressor mass flow rate (kg/s).
    pub mass_flow_mc_kg_per_s: f64,
    /// Recompressor mass flow rate (kg/s).
    pub mass_flow_rc_kg_per_s: f64,

    /// Main compressor power (MW).
    pub mc_power_mw: f64,
    /// Recompressor power (MW).
    pub rc_power_mw: f64,
    /// Turbine power (MW).
    pub turbine_power_mw: f64,
    /// Net power (MW).
    pub net_power_mw: f64,

    /// Primary HX heat input (MW).
    pub heat_input_mw: f64,
    /// Precooler heat rejection (MW).
    pub heat_rejection_mw: f64,
    /// Thermal efficiency (dimensionless).
    pub thermal_efficiency: f64,

    /// LT recuperator heat transfer (MW).
    pub lt_recuperator_heat_transfer_mw: f64,
    /// LT recuperator min ΔT (K).
    pub lt_recuperator_min_delta_t_k: f64,
    /// LT recuperator effectiveness (dimensionless).
    ///
    /// `None` if the pinch-point calculation did not converge.
    pub lt_recuperator_effectiveness: Option<f64>,

    /// HT recuperator heat transfer (MW).
    pub ht_recuperator_heat_transfer_mw: f64,
    /// HT recuperator min ΔT (K).
    pub ht_recuperator_min_delta_t_k: f64,
    /// HT recuperator effectiveness (dimensionless).
    ///
    /// `None` if the pinch-point calculation did not converge.
    pub ht_recuperator_effectiveness: Option<f64>,

    /// Thermodynamic states at the 10 cycle points.
    pub states: [StatePoint; 10],
}

#[cfg(feature = "wasm")]
fn default_model() -> String {
    String::from("CoolProp")
}

/// Run a recompression Brayton cycle design-point calculation.
///
/// # Errors
///
/// Returns a descriptive error string on invalid input or solver failure.
pub fn recomp_design_point(
    input: &RecompDesignPointInput,
) -> Result<RecompDesignPointOutput, String> {
    use twine_models::support::thermo::{
        fluid::CarbonDioxide,
        model::{CoolProp, PerfectGas},
    };

    use crate::fluids::{Butane, Helium, Nitrogen};

    fn solve_coolprop<
        F: Clone + Default + twine_models::support::thermo::model::coolprop::CoolPropFluid,
    >(
        op: OperatingPoint,
        recomp_frac: Ratio,
        config: &Config,
    ) -> Result<RecompDesignPointOutput, String> {
        let thermo = CoolProp::<F>::new()
            .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
        let solution = recompression::design_point(op, recomp_frac, config, F::default(), &thermo)
            .map_err(|e| e.to_string())?;
        Ok(convert_output(&solution, config, &thermo))
    }

    let (op, recomp_frac, config) = convert_input(input)?;

    match input.model.as_str() {
        "PerfectGas" => {
            let thermo = PerfectGas::<CarbonDioxide>::new()
                .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
            let solution =
                recompression::design_point(op, recomp_frac, &config, CarbonDioxide, &thermo)
                    .map_err(|e| e.to_string())?;
            Ok(convert_output(&solution, &config, &thermo))
        }
        "CoolProp" => match input.fluid.as_str() {
            "CarbonDioxide" => solve_coolprop::<CarbonDioxide>(op, recomp_frac, &config),
            "Nitrogen" => solve_coolprop::<Nitrogen>(op, recomp_frac, &config),
            "Helium" => solve_coolprop::<Helium>(op, recomp_frac, &config),
            "Butane" => solve_coolprop::<Butane>(op, recomp_frac, &config),
            other => Err(format!("unknown fluid: {other}")),
        },
        other => Err(format!("unknown model: {other}")),
    }
}

#[allow(clippy::similar_names)] // mc/rc/lt/ht are standard domain abbreviations.
fn convert_input(
    input: &RecompDesignPointInput,
) -> Result<(OperatingPoint, Ratio, Config), String> {
    let op = OperatingPoint {
        t_comp_in: ThermodynamicTemperature::new::<degree_celsius>(input.compressor_inlet_temp_c),
        t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(input.turbine_inlet_temp_c),
        p_comp_in: Pressure::new::<megapascal>(input.compressor_inlet_pressure_mpa),
        p_comp_out: Pressure::new::<megapascal>(input.compressor_outlet_pressure_mpa),
        net_power: Power::new::<megawatt>(input.net_power_mw),
    };

    let f = input.recomp_frac;
    if !(0.0..1.0).contains(&f) {
        return Err(format!("invalid recomp_frac: {f} (must be in [0, 1))"));
    }
    let recomp_frac = Ratio::new::<ratio>(f);

    let eta_mc = IsentropicEfficiency::new(input.mc_efficiency)
        .map_err(|e| format!("invalid mc_efficiency: {e}"))?;
    let eta_rc = IsentropicEfficiency::new(input.rc_efficiency)
        .map_err(|e| format!("invalid rc_efficiency: {e}"))?;
    let eta_turb = IsentropicEfficiency::new(input.turbine_efficiency)
        .map_err(|e| format!("invalid turbine_efficiency: {e}"))?;

    let dp_lt_cold =
        PressureDrop::fraction(Ratio::new::<ratio>(input.lt_recuperator_dp_cold_fraction))
            .map_err(|e| format!("invalid lt_recuperator_dp_cold_fraction: {e}"))?;
    let dp_lt_hot =
        PressureDrop::fraction(Ratio::new::<ratio>(input.lt_recuperator_dp_hot_fraction))
            .map_err(|e| format!("invalid lt_recuperator_dp_hot_fraction: {e}"))?;
    let dp_ht_cold =
        PressureDrop::fraction(Ratio::new::<ratio>(input.ht_recuperator_dp_cold_fraction))
            .map_err(|e| format!("invalid ht_recuperator_dp_cold_fraction: {e}"))?;
    let dp_ht_hot =
        PressureDrop::fraction(Ratio::new::<ratio>(input.ht_recuperator_dp_hot_fraction))
            .map_err(|e| format!("invalid ht_recuperator_dp_hot_fraction: {e}"))?;
    let dp_precooler = PressureDrop::fraction(Ratio::new::<ratio>(input.precooler_dp_fraction))
        .map_err(|e| format!("invalid precooler_dp_fraction: {e}"))?;
    let dp_primary = PressureDrop::fraction(Ratio::new::<ratio>(input.primary_hx_dp_fraction))
        .map_err(|e| format!("invalid primary_hx_dp_fraction: {e}"))?;

    let config = Config {
        turbo: TurboConfig {
            eta_mc,
            eta_rc,
            eta_turb,
        },
        hx: HxConfig {
            lt_recuperator: RecuperatorConfig {
                ua: ThermalConductance::new::<kilowatt_per_kelvin>(
                    input.lt_recuperator_ua_kw_per_k,
                ),
                segments: input.lt_recuperator_segments,
                dp_cold: dp_lt_cold,
                dp_hot: dp_lt_hot,
            },
            ht_recuperator: RecuperatorConfig {
                ua: ThermalConductance::new::<kilowatt_per_kelvin>(
                    input.ht_recuperator_ua_kw_per_k,
                ),
                segments: input.ht_recuperator_segments,
                dp_cold: dp_ht_cold,
                dp_hot: dp_ht_hot,
            },
            precooler_dp: dp_precooler,
            primary_dp: dp_primary,
        },
        temp_tol: TemperatureInterval::new::<delta_kelvin>(1e-6),
    };

    Ok((op, recomp_frac, config))
}

fn convert_output<Fluid, Thermo>(
    solution: &recompression::Solution<Fluid>,
    config: &Config,
    thermo: &Thermo,
) -> RecompDesignPointOutput
where
    Fluid: Clone,
    Thermo: HasPressure<Fluid = Fluid>
        + HasEnthalpy<Fluid = Fluid>
        + HasEntropy<Fluid = Fluid>
        + EffectivenessThermo<Fluid>,
{
    let s = &solution.states;
    let state_array = [
        &s.s1, &s.s2, &s.s3, &s.s4, &s.s5, &s.s6, &s.s7, &s.s8, &s.s9, &s.s10,
    ];
    let points = state_array.map(|st| crate::thermo::state_to_point(st, thermo));

    let w_dot_net = solution.w_dot_turb - solution.w_dot_mc - solution.w_dot_rc;

    // Pressures from states for recuperator pressure drops.
    let p2 = thermo.pressure(&s.s2).expect("p2");
    let p3 = thermo.pressure(&s.s3).expect("p3");
    let p4 = thermo.pressure(&s.s4).expect("p4");
    let p5 = thermo.pressure(&s.s5).expect("p5");
    let p7 = thermo.pressure(&s.s7).expect("p7");
    let p8 = thermo.pressure(&s.s8).expect("p8");
    let p9 = thermo.pressure(&s.s9).expect("p9");

    // LT recuperator: top = cold (s2→s3), bottom = hot (s8→s9).
    let lt_effectiveness = effectiveness::compute(
        Inlets {
            top: s.s2.clone(),
            bottom: s.s8.clone(),
        },
        MassFlows::new_unchecked(solution.m_dot_mc, solution.m_dot_t),
        PressureDrops::new_unchecked(p2 - p3, p8 - p9),
        solution.q_dot_lt,
        config.hx.lt_recuperator.segments,
        thermo,
    );

    // HT recuperator: top = cold (s4→s5), bottom = hot (s7→s8).
    let ht_effectiveness = effectiveness::compute(
        Inlets {
            top: s.s4.clone(),
            bottom: s.s7.clone(),
        },
        MassFlows::new_unchecked(solution.m_dot_t, solution.m_dot_t),
        PressureDrops::new_unchecked(p4 - p5, p7 - p8),
        solution.q_dot_ht,
        config.hx.ht_recuperator.segments,
        thermo,
    );

    RecompDesignPointOutput {
        mass_flow_total_kg_per_s: solution.m_dot_t.get::<kilogram_per_second>(),
        mass_flow_mc_kg_per_s: solution.m_dot_mc.get::<kilogram_per_second>(),
        mass_flow_rc_kg_per_s: solution.m_dot_rc.get::<kilogram_per_second>(),
        mc_power_mw: solution.w_dot_mc.get::<megawatt>(),
        rc_power_mw: solution.w_dot_rc.get::<megawatt>(),
        turbine_power_mw: solution.w_dot_turb.get::<megawatt>(),
        net_power_mw: w_dot_net.get::<megawatt>(),
        heat_input_mw: solution.q_dot_phx.get::<megawatt>(),
        heat_rejection_mw: solution.q_dot_pc.get::<megawatt>(),
        thermal_efficiency: solution.eta_thermal.get::<ratio>(),
        lt_recuperator_heat_transfer_mw: solution.q_dot_lt.get::<megawatt>(),
        lt_recuperator_min_delta_t_k: solution.lt_min_delta_t.get::<delta_kelvin>(),
        lt_recuperator_effectiveness: lt_effectiveness,
        ht_recuperator_heat_transfer_mw: solution.q_dot_ht.get::<megawatt>(),
        ht_recuperator_min_delta_t_k: solution.ht_min_delta_t.get::<delta_kelvin>(),
        ht_recuperator_effectiveness: ht_effectiveness,
        states: points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_input() -> RecompDesignPointInput {
        RecompDesignPointInput {
            model: String::from("CoolProp"),
            fluid: String::from("CarbonDioxide"),
            compressor_inlet_temp_c: 32.0,
            turbine_inlet_temp_c: 550.0,
            compressor_inlet_pressure_mpa: 7.7,
            compressor_outlet_pressure_mpa: 20.0,
            net_power_mw: 10.0,
            recomp_frac: 0.3,
            mc_efficiency: 0.89,
            rc_efficiency: 0.89,
            turbine_efficiency: 0.93,
            lt_recuperator_ua_kw_per_k: 5000.0,
            lt_recuperator_segments: 10,
            lt_recuperator_dp_cold_fraction: 0.0,
            lt_recuperator_dp_hot_fraction: 0.0,
            ht_recuperator_ua_kw_per_k: 5000.0,
            ht_recuperator_segments: 10,
            ht_recuperator_dp_cold_fraction: 0.0,
            ht_recuperator_dp_hot_fraction: 0.0,
            precooler_dp_fraction: 0.0,
            primary_hx_dp_fraction: 0.0,
        }
    }

    #[test]
    fn smoke_test() {
        let result = recomp_design_point(&baseline_input());
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let out = result.unwrap();

        assert!(out.mass_flow_total_kg_per_s > 0.0);
        assert!(out.turbine_power_mw > out.mc_power_mw + out.rc_power_mw);
        assert!(out.thermal_efficiency > 0.0);
        assert!(out.thermal_efficiency < 1.0);

        let relative_error = (out.net_power_mw - 10.0).abs() / 10.0;
        assert!(
            relative_error < 1e-4,
            "net power {:.6} MW deviates from target",
            out.net_power_mw,
        );

        // Effectiveness must be physical.
        let lt_eff = out
            .lt_recuperator_effectiveness
            .expect("LT effectiveness should converge for baseline");
        assert!(
            (0.0..=1.0).contains(&lt_eff),
            "LT effectiveness {lt_eff} is outside [0, 1]",
        );
        let ht_eff = out
            .ht_recuperator_effectiveness
            .expect("HT effectiveness should converge for baseline");
        assert!(
            (0.0..=1.0).contains(&ht_eff),
            "HT effectiveness {ht_eff} is outside [0, 1]",
        );
    }

    #[test]
    fn all_state_points_valid() {
        let out = recomp_design_point(&baseline_input()).unwrap();
        for (i, state) in out.states.iter().enumerate() {
            assert!(!state.temperature_c.is_nan(), "state {i} temperature NaN");
            assert!(state.pressure_mpa > 0.0, "state {i} pressure invalid");
            assert!(state.density_kg_per_m3 > 0.0, "state {i} density invalid");
        }
    }

    #[test]
    fn unknown_fluid_returns_error() {
        let input = RecompDesignPointInput {
            fluid: String::from("Unobtanium"),
            ..baseline_input()
        };
        let result = recomp_design_point(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown fluid"));
    }
}

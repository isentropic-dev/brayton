use brayton::{
    IsentropicEfficiency, OperatingPoint, PressureDrop, RecuperatorConfig,
    simple::{Config, Error, HxConfig, TurboConfig, design_point},
};

use twine_models::support::thermo::{fluid::CarbonDioxide, model::PerfectGas};
use uom::si::{
    f64::{Power, Pressure, Ratio, ThermalConductance, ThermodynamicTemperature},
    power::megawatt,
    pressure::kilopascal,
    ratio::ratio,
    thermal_conductance::kilowatt_per_kelvin,
    thermodynamic_temperature::degree_celsius,
};

/// Create a reasonable baseline configuration for testing.
fn baseline_config() -> Config {
    Config {
        turbo: TurboConfig {
            eta_comp: IsentropicEfficiency::new(0.89).unwrap(),
            eta_turb: IsentropicEfficiency::new(0.93).unwrap(),
        },
        hx: HxConfig {
            recuperator: RecuperatorConfig {
                ua: ThermalConductance::new::<kilowatt_per_kelvin>(2000.0),
                segments: 10,
                dp_cold: PressureDrop::fraction(Ratio::new::<ratio>(0.02)).unwrap(),
                dp_hot: PressureDrop::fraction(Ratio::new::<ratio>(0.02)).unwrap(),
            },
            precooler_dp: PressureDrop::fraction(Ratio::new::<ratio>(0.01)).unwrap(),
            primary_dp: PressureDrop::fraction(Ratio::new::<ratio>(0.01)).unwrap(),
        },
    }
}

/// Create a reasonable baseline operating point for testing.
fn baseline_operating_point() -> OperatingPoint {
    OperatingPoint {
        t_comp_in: ThermodynamicTemperature::new::<degree_celsius>(50.0),
        t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(500.0),
        p_comp_in: Pressure::new::<kilopascal>(100.0),
        p_comp_out: Pressure::new::<kilopascal>(300.0),
        net_power: Power::new::<megawatt>(10.0),
    }
}

#[test]
fn smoke_test_perfect_gas_co2() {
    let op = baseline_operating_point();
    let config = baseline_config();
    let fluid = CarbonDioxide;
    let thermo = PerfectGas::<CarbonDioxide>::new().unwrap();

    let result = design_point(op, &config, fluid, &thermo);

    assert!(result.is_ok());
    let solution = result.unwrap();

    // Verify basic physical constraints.
    assert!(solution.m_dot.value > 0.0);
    assert!(solution.w_dot_comp.value > 0.0);
    assert!(solution.w_dot_turb.value > solution.w_dot_comp.value);
    assert!(solution.q_dot_phx.value > 0.0);
    assert!(solution.q_dot_pc.value > 0.0);
    assert!(solution.eta_thermal.value > 0.0);
    assert!(solution.eta_thermal.value < 1.0);
}

#[test]
fn design_point_insufficient_pressure_rise() {
    // Low compression ratio.
    let op = OperatingPoint {
        p_comp_out: Pressure::new::<kilopascal>(105.0),
        ..baseline_operating_point()
    };

    let config = baseline_config();
    let thermo = PerfectGas::<CarbonDioxide>::new().unwrap();

    let result = design_point(op, &config, CarbonDioxide, &thermo);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        Error::InsufficientPressureRise { .. }
    ));
}

#[test]
fn design_point_insufficient_turbine_work() {
    // Turbine inlet temperature too low relative to compressor inlet.
    let op = OperatingPoint {
        t_turb_in: ThermodynamicTemperature::new::<degree_celsius>(250.0),
        ..baseline_operating_point()
    };

    let config = baseline_config();
    let thermo = PerfectGas::<CarbonDioxide>::new().unwrap();

    let result = design_point(op, &config, CarbonDioxide, &thermo);

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        Error::InsufficientTurbineWork { .. }
    ));
}

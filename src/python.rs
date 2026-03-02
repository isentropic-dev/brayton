use pyo3::{exceptions::PyValueError, prelude::*};

use crate::facade;

/// Thermodynamic state at a single cycle point.
#[pyclass]
pub struct StatePoint {
    /// Temperature in degrees Celsius.
    #[pyo3(get)]
    pub temperature_c: f64,

    /// Pressure in kilopascals.
    #[pyo3(get)]
    pub pressure_kpa: f64,

    /// Mass density in kilograms per cubic metre.
    #[pyo3(get)]
    pub density_kg_per_m3: f64,

    /// Specific enthalpy in kilojoules per kilogram.
    #[pyo3(get)]
    pub enthalpy_kj_per_kg: f64,

    /// Specific entropy in kilojoules per kilogram-kelvin.
    #[pyo3(get)]
    pub entropy_kj_per_kg_k: f64,
}

impl From<facade::StatePoint> for StatePoint {
    fn from(s: facade::StatePoint) -> Self {
        Self {
            temperature_c: s.temperature_c,
            pressure_kpa: s.pressure_kpa,
            density_kg_per_m3: s.density_kg_per_m3,
            enthalpy_kj_per_kg: s.enthalpy_kj_per_kg,
            entropy_kj_per_kg_k: s.entropy_kj_per_kg_k,
        }
    }
}

/// Thermodynamic performance output for a design-point calculation.
#[pyclass]
pub struct DesignPointResult {
    /// Cycle mass flow rate in kilograms per second.
    #[pyo3(get)]
    pub mass_flow_kg_per_s: f64,

    /// Compressor power consumption in kilowatts.
    #[pyo3(get)]
    pub compressor_power_kw: f64,

    /// Turbine power output in kilowatts.
    #[pyo3(get)]
    pub turbine_power_kw: f64,

    /// Net cycle power output (turbine minus compressor) in kilowatts.
    #[pyo3(get)]
    pub net_power_kw: f64,

    /// Primary heat exchanger heat addition rate in kilowatts.
    #[pyo3(get)]
    pub heat_input_kw: f64,

    /// Precooler heat rejection rate in kilowatts.
    #[pyo3(get)]
    pub heat_rejection_kw: f64,

    /// Cycle thermal efficiency (`η = W_net / Q_in`), dimensionless.
    #[pyo3(get)]
    pub thermal_efficiency: f64,

    /// Thermodynamic states at the six cycle points.
    states_inner: Vec<StatePoint>,
}

#[pymethods]
impl DesignPointResult {
    /// Thermodynamic states at the six cycle points.
    #[getter]
    pub fn states(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let list = pyo3::types::PyList::empty(py);
        for s in &self.states_inner {
            let obj = Py::new(
                py,
                StatePoint {
                    temperature_c: s.temperature_c,
                    pressure_kpa: s.pressure_kpa,
                    density_kg_per_m3: s.density_kg_per_m3,
                    enthalpy_kj_per_kg: s.enthalpy_kj_per_kg,
                    entropy_kj_per_kg_k: s.entropy_kj_per_kg_k,
                },
            )?;
            list.append(obj)?;
        }
        Ok(list.into())
    }
}

impl From<facade::DesignPointOutput> for DesignPointResult {
    fn from(o: facade::DesignPointOutput) -> Self {
        Self {
            mass_flow_kg_per_s: o.mass_flow_kg_per_s,
            compressor_power_kw: o.compressor_power_kw,
            turbine_power_kw: o.turbine_power_kw,
            net_power_kw: o.net_power_kw,
            heat_input_kw: o.heat_input_kw,
            heat_rejection_kw: o.heat_rejection_kw,
            thermal_efficiency: o.thermal_efficiency,
            states_inner: o.states.into_iter().map(StatePoint::from).collect(),
        }
    }
}

/// Run a simple recuperated Brayton cycle design-point calculation.
///
/// All temperatures in °C, pressures in kPa, power in kW, thermal
/// conductance in kW/K. Efficiencies and pressure-drop fractions are
/// dimensionless ratios (0–1).
///
/// Returns a `DesignPointResult` on success, or raises `ValueError` on
/// invalid input or solver failure.
#[pyfunction]
#[pyo3(signature = (
    compressor_inlet_temp_c,
    turbine_inlet_temp_c,
    compressor_inlet_pressure_kpa,
    compressor_outlet_pressure_kpa,
    net_power_kw,
    compressor_efficiency,
    turbine_efficiency,
    recuperator_ua_kw_per_k,
    recuperator_segments,
    recuperator_dp_cold_fraction,
    recuperator_dp_hot_fraction,
    precooler_dp_fraction,
    primary_hx_dp_fraction,
))]
#[allow(clippy::too_many_arguments)]
fn design_point(
    compressor_inlet_temp_c: f64,
    turbine_inlet_temp_c: f64,
    compressor_inlet_pressure_kpa: f64,
    compressor_outlet_pressure_kpa: f64,
    net_power_kw: f64,
    compressor_efficiency: f64,
    turbine_efficiency: f64,
    recuperator_ua_kw_per_k: f64,
    recuperator_segments: usize,
    recuperator_dp_cold_fraction: f64,
    recuperator_dp_hot_fraction: f64,
    precooler_dp_fraction: f64,
    primary_hx_dp_fraction: f64,
) -> PyResult<DesignPointResult> {
    let input = facade::DesignPointInput {
        compressor_inlet_temp_c,
        turbine_inlet_temp_c,
        compressor_inlet_pressure_kpa,
        compressor_outlet_pressure_kpa,
        net_power_kw,
        compressor_efficiency,
        turbine_efficiency,
        recuperator_ua_kw_per_k,
        recuperator_segments,
        recuperator_dp_cold_fraction,
        recuperator_dp_hot_fraction,
        precooler_dp_fraction,
        primary_hx_dp_fraction,
    };

    let output = facade::design_point(&input).map_err(PyValueError::new_err)?;
    Ok(DesignPointResult::from(output))
}

/// Python module for the Brayton cycle design-point solver.
#[pymodule]
fn brayton(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(design_point, m)?)?;
    m.add_class::<DesignPointResult>()?;
    m.add_class::<StatePoint>()?;
    Ok(())
}

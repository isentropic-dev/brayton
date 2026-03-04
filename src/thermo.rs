//! Thermodynamic state construction helpers.
//!
//! `state_to_point` is used by the facade for all builds.
//! The batch construction helpers (`states_from_ph`, `states_from_ps`) are
//! exposed only through the WASM interface.

use twine_models::support::thermo::{
    State,
    capability::{HasEnthalpy, HasEntropy, HasPressure},
};
use uom::si::{
    available_energy::kilojoule_per_kilogram, mass_density::kilogram_per_cubic_meter,
    pressure::megapascal, specific_heat_capacity::kilojoule_per_kilogram_kelvin,
    thermodynamic_temperature::degree_celsius,
};

use crate::StatePoint;

#[cfg(feature = "wasm")]
use twine_models::support::{
    thermo::{
        capability::StateFrom,
        fluid::CarbonDioxide,
        model::{CoolProp, PerfectGas},
    },
    units::{SpecificEnthalpy, SpecificEntropy},
};

#[cfg(feature = "wasm")]
use uom::si::f64::Pressure;

#[cfg(feature = "wasm")]
use crate::fluids::{Butane, Helium, Nitrogen};

/// Input for batch state construction from pressure and enthalpy arrays.
#[cfg(feature = "wasm")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StatesFromPhInput {
    /// Thermodynamic model (`"PerfectGas"` or `"CoolProp"`).
    #[serde(default = "default_model")]
    pub model: String,

    /// Working fluid (ignored for `PerfectGas`).
    #[serde(default = "default_fluid")]
    pub fluid: String,

    /// Pressures in megapascals.
    pub pressures_mpa: Vec<f64>,

    /// Specific enthalpies in kilojoules per kilogram.
    pub enthalpies_kj_per_kg: Vec<f64>,
}

/// Input for batch state construction from pressure and entropy arrays.
#[cfg(feature = "wasm")]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct StatesFromPsInput {
    /// Thermodynamic model (`"PerfectGas"` or `"CoolProp"`).
    #[serde(default = "default_model")]
    pub model: String,

    /// Working fluid (ignored for `PerfectGas`).
    #[serde(default = "default_fluid")]
    pub fluid: String,

    /// Pressures in megapascals.
    pub pressures_mpa: Vec<f64>,

    /// Specific entropies in kilojoules per kilogram-kelvin.
    pub entropies_kj_per_kg_k: Vec<f64>,
}

/// Construct states from arrays of pressure and enthalpy.
///
/// # Errors
///
/// Returns a descriptive error string on mismatched array lengths,
/// unknown model/fluid, or thermodynamic model failure.
#[cfg(feature = "wasm")]
pub fn states_from_ph(input: &StatesFromPhInput) -> Result<Vec<StatePoint>, String> {
    if input.pressures_mpa.len() != input.enthalpies_kj_per_kg.len() {
        return Err(format!(
            "array length mismatch: {} pressures, {} enthalpies",
            input.pressures_mpa.len(),
            input.enthalpies_kj_per_kg.len(),
        ));
    }

    let pressures: Vec<Pressure> = input
        .pressures_mpa
        .iter()
        .map(|&p| Pressure::new::<megapascal>(p))
        .collect();
    let enthalpies: Vec<SpecificEnthalpy> = input
        .enthalpies_kj_per_kg
        .iter()
        .map(|&h| SpecificEnthalpy::new::<kilojoule_per_kilogram>(h))
        .collect();

    match input.model.as_str() {
        "PerfectGas" => {
            reject_non_co2_perfect_gas(&input.fluid)?;
            let thermo = PerfectGas::<CarbonDioxide>::new()
                .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
            batch_from_ph(&thermo, CarbonDioxide, &pressures, &enthalpies)
        }
        "CoolProp" => match input.fluid.as_str() {
            "CarbonDioxide" => batch_from_ph_coolprop::<CarbonDioxide>(&pressures, &enthalpies),
            "Nitrogen" => batch_from_ph_coolprop::<Nitrogen>(&pressures, &enthalpies),
            "Helium" => batch_from_ph_coolprop::<Helium>(&pressures, &enthalpies),
            "Butane" => batch_from_ph_coolprop::<Butane>(&pressures, &enthalpies),
            other => Err(format!("unknown fluid: {other}")),
        },
        other => Err(format!("unknown model: {other}")),
    }
}

/// Construct states from arrays of pressure and entropy.
///
/// # Errors
///
/// Returns a descriptive error string on mismatched array lengths,
/// unknown model/fluid, or thermodynamic model failure.
#[cfg(feature = "wasm")]
pub fn states_from_ps(input: &StatesFromPsInput) -> Result<Vec<StatePoint>, String> {
    if input.pressures_mpa.len() != input.entropies_kj_per_kg_k.len() {
        return Err(format!(
            "array length mismatch: {} pressures, {} entropies",
            input.pressures_mpa.len(),
            input.entropies_kj_per_kg_k.len(),
        ));
    }

    let pressures: Vec<Pressure> = input
        .pressures_mpa
        .iter()
        .map(|&p| Pressure::new::<megapascal>(p))
        .collect();
    let entropies: Vec<SpecificEntropy> = input
        .entropies_kj_per_kg_k
        .iter()
        .map(|&s| SpecificEntropy::new::<kilojoule_per_kilogram_kelvin>(s))
        .collect();

    match input.model.as_str() {
        "PerfectGas" => {
            reject_non_co2_perfect_gas(&input.fluid)?;
            let thermo = PerfectGas::<CarbonDioxide>::new()
                .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
            batch_from_ps(&thermo, CarbonDioxide, &pressures, &entropies)
        }
        "CoolProp" => match input.fluid.as_str() {
            "CarbonDioxide" => batch_from_ps_coolprop::<CarbonDioxide>(&pressures, &entropies),
            "Nitrogen" => batch_from_ps_coolprop::<Nitrogen>(&pressures, &entropies),
            "Helium" => batch_from_ps_coolprop::<Helium>(&pressures, &entropies),
            "Butane" => batch_from_ps_coolprop::<Butane>(&pressures, &entropies),
            other => Err(format!("unknown fluid: {other}")),
        },
        other => Err(format!("unknown model: {other}")),
    }
}

/// Rejects non-CO₂ fluids for `PerfectGas`, which only supports carbon dioxide.
#[cfg(feature = "wasm")]
fn reject_non_co2_perfect_gas(fluid: &str) -> Result<(), String> {
    if fluid != "CarbonDioxide" {
        return Err(format!(
            "PerfectGas only supports CarbonDioxide, got \"{fluid}\""
        ));
    }
    Ok(())
}

/// Batch `state_from(P, h)` for a `CoolProp` fluid.
#[cfg(feature = "wasm")]
fn batch_from_ph_coolprop<F>(
    pressures: &[Pressure],
    enthalpies: &[SpecificEnthalpy],
) -> Result<Vec<StatePoint>, String>
where
    F: Clone + Default + twine_models::support::thermo::model::coolprop::CoolPropFluid,
{
    let thermo = CoolProp::<F>::new()
        .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
    batch_from_ph(&thermo, F::default(), pressures, enthalpies)
}

/// Batch `state_from(P, s)` for a `CoolProp` fluid.
#[cfg(feature = "wasm")]
fn batch_from_ps_coolprop<F>(
    pressures: &[Pressure],
    entropies: &[SpecificEntropy],
) -> Result<Vec<StatePoint>, String>
where
    F: Clone + Default + twine_models::support::thermo::model::coolprop::CoolPropFluid,
{
    let thermo = CoolProp::<F>::new()
        .map_err(|e| format!("failed to construct thermodynamic model: {e}"))?;
    batch_from_ps(&thermo, F::default(), pressures, entropies)
}

/// Batch `state_from(P, h)` with a generic thermo model.
#[cfg(feature = "wasm")]
fn batch_from_ph<Fluid, Thermo>(
    thermo: &Thermo,
    fluid: Fluid,
    pressures: &[Pressure],
    enthalpies: &[SpecificEnthalpy],
) -> Result<Vec<StatePoint>, String>
where
    Fluid: Clone,
    Thermo: HasPressure<Fluid = Fluid>
        + HasEnthalpy<Fluid = Fluid>
        + HasEntropy<Fluid = Fluid>
        + StateFrom<(Fluid, Pressure, SpecificEnthalpy)>,
{
    pressures
        .iter()
        .zip(enthalpies)
        .enumerate()
        .map(|(i, (&p, &h))| {
            let state = thermo
                .state_from((fluid.clone(), p, h))
                .map_err(|e| format!("state_from(P, h) failed at index {i}: {e}"))?;
            Ok(state_to_point(&state, thermo))
        })
        .collect()
}

/// Batch `state_from(P, s)` with a generic thermo model.
#[cfg(feature = "wasm")]
fn batch_from_ps<Fluid, Thermo>(
    thermo: &Thermo,
    fluid: Fluid,
    pressures: &[Pressure],
    entropies: &[SpecificEntropy],
) -> Result<Vec<StatePoint>, String>
where
    Fluid: Clone,
    Thermo: HasPressure<Fluid = Fluid>
        + HasEnthalpy<Fluid = Fluid>
        + HasEntropy<Fluid = Fluid>
        + StateFrom<(Fluid, Pressure, SpecificEntropy)>,
{
    pressures
        .iter()
        .zip(entropies)
        .enumerate()
        .map(|(i, (&p, &s))| {
            let state = thermo
                .state_from((fluid.clone(), p, s))
                .map_err(|e| format!("state_from(P, s) failed at index {i}: {e}"))?;
            Ok(state_to_point(&state, thermo))
        })
        .collect()
}

/// Convert a thermodynamic state to a [`StatePoint`].
pub(crate) fn state_to_point<Fluid>(
    state: &State<Fluid>,
    thermo: &(impl HasPressure<Fluid = Fluid> + HasEnthalpy<Fluid = Fluid> + HasEntropy<Fluid = Fluid>),
) -> StatePoint {
    let t = state.temperature.get::<degree_celsius>();
    let rho = state.density.get::<kilogram_per_cubic_meter>();

    let pressure = thermo
        .pressure(state)
        .unwrap_or_else(|e| panic!("pressure undefined at T={t:.1}°C, ρ={rho:.1}: {e}"));
    let enthalpy = thermo
        .enthalpy(state)
        .unwrap_or_else(|e| panic!("enthalpy undefined at T={t:.1}°C, ρ={rho:.1}: {e}"));
    let entropy = thermo
        .entropy(state)
        .unwrap_or_else(|e| panic!("entropy undefined at T={t:.1}°C, ρ={rho:.1}: {e}"));

    StatePoint {
        temperature_c: t,
        pressure_mpa: pressure.get::<megapascal>(),
        density_kg_per_m3: rho,
        enthalpy_kj_per_kg: enthalpy.get::<kilojoule_per_kilogram>(),
        entropy_kj_per_kg_k: entropy.get::<kilojoule_per_kilogram_kelvin>(),
    }
}

#[cfg(feature = "wasm")]
fn default_model() -> String {
    String::from("PerfectGas")
}

#[cfg(feature = "wasm")]
fn default_fluid() -> String {
    String::from("CarbonDioxide")
}

#[cfg(feature = "wasm")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_from_ph_perfect_gas() {
        let input = StatesFromPhInput {
            model: String::from("PerfectGas"),
            fluid: String::from("CarbonDioxide"),
            pressures_mpa: vec![0.1, 0.2, 0.3],
            enthalpies_kj_per_kg: vec![300.0, 350.0, 400.0],
        };
        let result = states_from_ph(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let states = result.unwrap();
        assert_eq!(states.len(), 3);
        for (i, s) in states.iter().enumerate() {
            assert!(!s.temperature_c.is_nan(), "state {i} temperature is NaN");
            assert!(s.pressure_mpa > 0.0, "state {i} pressure must be positive");
        }
    }

    #[test]
    fn states_from_ps_perfect_gas() {
        let input = StatesFromPsInput {
            model: String::from("PerfectGas"),
            fluid: String::from("CarbonDioxide"),
            pressures_mpa: vec![0.1, 0.2, 0.3],
            entropies_kj_per_kg_k: vec![1.0, 1.0, 1.0],
        };
        let result = states_from_ps(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        let states = result.unwrap();
        assert_eq!(states.len(), 3);
    }

    #[test]
    fn mismatched_array_lengths_returns_error() {
        let input = StatesFromPhInput {
            model: String::from("PerfectGas"),
            fluid: String::from("CarbonDioxide"),
            pressures_mpa: vec![0.1, 0.2],
            enthalpies_kj_per_kg: vec![300.0],
        };
        let result = states_from_ph(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("array length mismatch"));
    }

    #[test]
    fn unknown_model_returns_error() {
        let input = StatesFromPhInput {
            model: String::from("Bogus"),
            fluid: String::from("CarbonDioxide"),
            pressures_mpa: vec![0.1],
            enthalpies_kj_per_kg: vec![300.0],
        };
        assert!(states_from_ph(&input).is_err());
    }

    #[test]
    fn states_from_ph_coolprop() {
        let input = StatesFromPhInput {
            model: String::from("CoolProp"),
            fluid: String::from("CarbonDioxide"),
            pressures_mpa: vec![8.0, 12.0, 20.0],
            enthalpies_kj_per_kg: vec![400.0, 450.0, 500.0],
        };
        let result = states_from_ph(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn states_from_ps_coolprop() {
        let input = StatesFromPsInput {
            model: String::from("CoolProp"),
            fluid: String::from("CarbonDioxide"),
            pressures_mpa: vec![8.0, 12.0, 20.0],
            entropies_kj_per_kg_k: vec![1.5, 1.5, 1.5],
        };
        let result = states_from_ps(&input);
        assert!(result.is_ok(), "expected Ok, got: {:?}", result.err());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn unknown_fluid_returns_error() {
        let input = StatesFromPhInput {
            model: String::from("CoolProp"),
            fluid: String::from("Unobtanium"),
            pressures_mpa: vec![0.1],
            enthalpies_kj_per_kg: vec![300.0],
        };
        let result = states_from_ph(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown fluid"));
    }

    #[test]
    fn perfect_gas_rejects_non_co2() {
        let input = StatesFromPhInput {
            model: String::from("PerfectGas"),
            fluid: String::from("Helium"),
            pressures_mpa: vec![0.1],
            enthalpies_kj_per_kg: vec![300.0],
        };
        let result = states_from_ph(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("PerfectGas only supports"));
    }
}

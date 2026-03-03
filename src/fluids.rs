//! Additional fluid types for CoolProp-backed calculations.
//!
//! These fluids are defined here rather than in `twine-models` because
//! they're only needed for `CoolProp` dispatch in the brayton dashboard.
//! Each type implements [`CoolPropFluid`] with the `CoolProp` backend name.

use twine_models::support::thermo::model::coolprop::CoolPropFluid;

/// Canonical identifier for nitrogen (N₂).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Nitrogen;

impl CoolPropFluid for Nitrogen {
    const BACKEND: &'static str = "HEOS";
    const NAME: &'static str = "Nitrogen";
}

/// Canonical identifier for helium (He).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Helium;

impl CoolPropFluid for Helium {
    const BACKEND: &'static str = "HEOS";
    const NAME: &'static str = "Helium";
}

/// Canonical identifier for butane (C₄H₁₀).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Butane;

impl CoolPropFluid for Butane {
    const BACKEND: &'static str = "HEOS";
    const NAME: &'static str = "Butane";
}

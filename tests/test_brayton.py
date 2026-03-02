import pytest
from brayton import design_point


def baseline():
    return design_point(
        compressor_inlet_temp_c=50.0,
        turbine_inlet_temp_c=500.0,
        compressor_inlet_pressure_kpa=100.0,
        compressor_outlet_pressure_kpa=300.0,
        net_power_kw=10000.0,
        compressor_efficiency=0.89,
        turbine_efficiency=0.93,
        recuperator_ua_kw_per_k=2000.0,
        recuperator_segments=10,
        recuperator_dp_cold_fraction=0.02,
        recuperator_dp_hot_fraction=0.02,
        precooler_dp_fraction=0.01,
        primary_hx_dp_fraction=0.01,
    )


def test_smoke():
    result = baseline()
    assert result.mass_flow_kg_per_s > 0
    assert result.turbine_power_kw > result.compressor_power_kw
    assert 0 < result.thermal_efficiency < 1


def test_states_count():
    result = baseline()
    assert len(result.states) == 6


def test_states_valid():
    result = baseline()
    for s in result.states:
        assert s.pressure_kpa > 0
        assert s.density_kg_per_m3 > 0


def test_invalid_efficiency():
    with pytest.raises(ValueError, match="compressor_efficiency"):
        design_point(
            compressor_inlet_temp_c=50.0,
            turbine_inlet_temp_c=500.0,
            compressor_inlet_pressure_kpa=100.0,
            compressor_outlet_pressure_kpa=300.0,
            net_power_kw=10000.0,
            compressor_efficiency=1.5,
            turbine_efficiency=0.93,
            recuperator_ua_kw_per_k=2000.0,
            recuperator_segments=10,
            recuperator_dp_cold_fraction=0.02,
            recuperator_dp_hot_fraction=0.02,
            precooler_dp_fraction=0.01,
            primary_hx_dp_fraction=0.01,
        )

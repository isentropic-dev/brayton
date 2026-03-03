import { init, designPoint } from './brayton.js';
import { createCycleChart } from './cycle-chart.js';

const STATE_LABELS = [
  'Compressor in',
  'Compressor out',
  'Recup cold out',
  'Turbine in',
  'Turbine out',
  'Recup hot out',
];

const INPUT_IDS = [
  'compressor_inlet_temp_c',
  'turbine_inlet_temp_c',
  'compressor_inlet_pressure_mpa',
  'compressor_outlet_pressure_mpa',
  'net_power_mw',
  'compressor_efficiency_pct',
  'turbine_efficiency_pct',
  'recuperator_ua_kw_per_k',
  'recuperator_segments',
  'recuperator_dp_cold_pct',
  'recuperator_dp_hot_pct',
  'precooler_dp_pct',
  'primary_hx_dp_pct',
];

let tsChart = null;
let hsChart = null;
let pvChart = null;
let phChart = null;

// Fields where the UI shows percent but the facade expects a 0–1 fraction.
// Fields where the UI shows percent but the facade expects a 0–1 value.
const PCT_FIELDS = [
  'compressor_efficiency_pct',
  'turbine_efficiency_pct',
  'recuperator_dp_cold_pct',
  'recuperator_dp_hot_pct',
  'precooler_dp_pct',
  'primary_hx_dp_pct',
];

// Map from UI element IDs to facade field names (only where they differ).
const FIELD_MAP = {
  'compressor_efficiency_pct': 'compressor_efficiency',
  'turbine_efficiency_pct': 'turbine_efficiency',
  'recuperator_dp_cold_pct': 'recuperator_dp_cold_fraction',
  'recuperator_dp_hot_pct': 'recuperator_dp_hot_fraction',
  'precooler_dp_pct': 'precooler_dp_fraction',
  'primary_hx_dp_pct': 'primary_hx_dp_fraction',
};

function readInputs() {
  const obj = {};
  obj.model = document.getElementById('model').value;
  obj.fluid = document.getElementById('fluid').value;
  for (const id of INPUT_IDS) {
    const el = document.getElementById(id);
    const val = id === 'recuperator_segments'
      ? parseInt(el.value, 10)
      : parseFloat(el.value);
    if (isNaN(val)) return null;

    const key = FIELD_MAP[id] || id;
    obj[key] = PCT_FIELDS.includes(id) ? val / 100 : val;
  }
  return obj;
}

function fmt(v, decimals = 2) {
  return v.toFixed(decimals);
}

function renderScalars(r) {
  document.getElementById('r-mass-flow').textContent = fmt(r.mass_flow_kg_per_s, 1);
  document.getElementById('r-comp-power').textContent = fmt(r.compressor_power_mw, 2);
  document.getElementById('r-turb-power').textContent = fmt(r.turbine_power_mw, 2);
  document.getElementById('r-net-power').textContent = fmt(r.net_power_mw, 2);
  document.getElementById('r-heat-in').textContent = fmt(r.heat_input_mw, 2);
  document.getElementById('r-heat-rej').textContent = fmt(r.heat_rejection_mw, 2);
  document.getElementById('r-eta').textContent = fmt(r.thermal_efficiency * 100, 2);
}

function renderStates(states) {
  const tbody = document.getElementById('state-tbody');
  tbody.innerHTML = '';
  states.forEach((s, i) => {
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td>${i + 1}</td>
      <td>${STATE_LABELS[i]}</td>
      <td>${fmt(s.temperature_c, 1)}</td>
      <td>${fmt(s.pressure_mpa, 2)}</td>
      <td>${fmt(s.density_kg_per_m3, 1)}</td>
      <td>${fmt(s.enthalpy_kj_per_kg, 1)}</td>
      <td>${fmt(s.entropy_kj_per_kg_k, 4)}</td>
    `;
    tbody.appendChild(tr);
  });
}

function toChartPoints(states, xKey, yKey) {
  return states.map((s, i) => ({
    x: s[xKey],
    y: s[yKey],
    label: String(i + 1),
  }));
}

function renderCharts(states) {
  const tsPoints = toChartPoints(states, 'entropy_kj_per_kg_k', 'temperature_c');
  const hsPoints = toChartPoints(states, 'entropy_kj_per_kg_k', 'enthalpy_kj_per_kg');
  const pvPoints = states.map((s, i) => ({
    x: 1 / s.density_kg_per_m3,
    y: s.pressure_mpa,
    label: String(i + 1),
  }));
  const phPoints = toChartPoints(states, 'enthalpy_kj_per_kg', 'pressure_mpa');

  if (!tsChart) {
    tsChart = createCycleChart(document.getElementById('chart-ts'), {
      title: 'T–s',
      xLabel: 's (kJ/kg·K)',
      yLabel: 'T (°C)',
    });
  }
  tsChart.update(tsPoints);

  if (!hsChart) {
    hsChart = createCycleChart(document.getElementById('chart-hs'), {
      title: 'h–s',
      xLabel: 's (kJ/kg·K)',
      yLabel: 'h (kJ/kg)',
    });
  }
  hsChart.update(hsPoints);

  if (!pvChart) {
    pvChart = createCycleChart(document.getElementById('chart-pv'), {
      title: 'P–v',
      xLabel: 'v (m³/kg)',
      yLabel: 'P (MPa)',
    });
  }
  pvChart.update(pvPoints);

  if (!phChart) {
    phChart = createCycleChart(document.getElementById('chart-ph'), {
      title: 'P–h',
      xLabel: 'h (kJ/kg)',
      yLabel: 'P (MPa)',
    });
  }
  phChart.update(phPoints);
}

function showError(msg) {
  const el = document.getElementById('error');
  el.textContent = msg;
  el.hidden = false;
  document.getElementById('results-content').style.opacity = '0.3';
}

function clearError() {
  document.getElementById('error').hidden = true;
  document.getElementById('results-content').style.opacity = '1';
}

function calculate() {
  const input = readInputs();
  if (!input) {
    showError('Some inputs are empty or invalid.');
    return;
  }

  try {
    const result = designPoint(input);
    clearError();
    renderScalars(result);
    renderStates(result.states);
    renderCharts(result.states);
  } catch (e) {
    showError(e.message || String(e));
  }
}

let debounceTimer = null;

function onInputChange() {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(calculate, 300);
}

const COOLPROP_FLUIDS = [
  { value: 'CarbonDioxide', label: 'Carbon Dioxide' },
  { value: 'Nitrogen', label: 'Nitrogen' },
  { value: 'Helium', label: 'Helium' },
  { value: 'Butane', label: 'Butane' },
];

const PERFECT_GAS_FLUIDS = [
  { value: 'CarbonDioxide', label: 'Carbon Dioxide' },
];

function onModelChange() {
  const fluidSelect = document.getElementById('fluid');
  const fluids = document.getElementById('model').value === 'CoolProp'
    ? COOLPROP_FLUIDS
    : PERFECT_GAS_FLUIDS;

  const prev = fluidSelect.value;
  fluidSelect.innerHTML = '';
  for (const f of fluids) {
    const opt = document.createElement('option');
    opt.value = f.value;
    opt.textContent = f.label;
    fluidSelect.appendChild(opt);
  }

  // Keep the previous selection if it's still available, otherwise default.
  if (fluids.some(f => f.value === prev)) {
    fluidSelect.value = prev;
  }

  onInputChange();
}

async function main() {
  await init();

  for (const id of INPUT_IDS) {
    const el = document.getElementById(id);
    el.addEventListener('input', onInputChange);
  }
  document.getElementById('model').addEventListener('change', onModelChange);
  document.getElementById('fluid').addEventListener('change', onInputChange);

  // Populate fluid dropdown for the initial model selection, then calculate.
  onModelChange();
}

main().catch(e => {
  showError('Failed to initialize WASM: ' + (e.message || e));
});

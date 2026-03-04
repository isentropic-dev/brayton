import { init, designPoint, statesFromPh, statesFromPs } from './brayton.js';
import { createCycleChart } from './cycle-chart.js';
import { createRecupChart } from './recup-chart.js';

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
let recupChart = null;

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
  document.getElementById('r-eta').textContent = fmt(r.thermal_efficiency * 100, 2);
  document.getElementById('r-mass-flow').textContent = fmt(r.mass_flow_kg_per_s, 1);
  document.getElementById('r-comp-power').textContent = fmt(r.compressor_power_mw, 2);
  document.getElementById('r-turb-power').textContent = fmt(r.turbine_power_mw, 2);
  document.getElementById('r-heat-in').textContent = fmt(r.heat_input_mw, 2);
  document.getElementById('r-heat-rej').textContent = fmt(r.heat_rejection_mw, 2);
  document.getElementById('r-recup-q').textContent = fmt(r.recuperator_heat_transfer_mw, 2);
  document.getElementById('r-recup-min-dt').textContent = fmt(r.recuperator_min_delta_t_k, 1);
  document.getElementById('r-recup-effectiveness').textContent = fmt(r.recuperator_effectiveness, 3);
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

const N_CURVE_POINTS = 30;

/**
 * Linearly interpolate N values between a and b (inclusive of both endpoints).
 */
function linspace(a, b, n) {
  const arr = new Array(n);
  for (let i = 0; i < n; i++) {
    arr[i] = a + (b - a) * i / (n - 1);
  }
  return arr;
}

/**
 * Build smooth curve points for all 6 process paths around the cycle.
 *
 * Returns an array of StatePoint objects (with `label` on the 6 state points)
 * ordered sequentially around the cycle: 1→2, 2→3, 3→4, 4→5, 5→6, 6→1.
 */
function buildCurvePoints(states, model, fluid) {
  const [s1, s2, s3, s4, s5, s6] = states;
  const n = N_CURVE_POINTS;
  const base = { model, fluid };

  // Each segment: [from, to, method].
  // "ph" segments use statesFromPh (isobaric-ish HXs, recuperator).
  // "ps" segments use statesFromPs (turbomachinery).
  const segments = [
    { from: s1, to: s2, method: 'ps', label_from: '1' },
    { from: s2, to: s3, method: 'ph', label_from: '2' },
    { from: s3, to: s4, method: 'ph', label_from: '3' },
    { from: s4, to: s5, method: 'ps', label_from: '4' },
    { from: s5, to: s6, method: 'ph', label_from: '5' },
    { from: s6, to: s1, method: 'ph', label_from: '6' },
  ];

  const allPoints = [];

  for (const seg of segments) {
    const pressures = linspace(seg.from.pressure_mpa, seg.to.pressure_mpa, n);

    let curveStates;
    if (seg.method === 'ph') {
      const enthalpies = linspace(
        seg.from.enthalpy_kj_per_kg,
        seg.to.enthalpy_kj_per_kg,
        n,
      );
      curveStates = statesFromPh({
        ...base,
        pressures_mpa: pressures,
        enthalpies_kj_per_kg: enthalpies,
      });
    } else {
      const entropies = linspace(
        seg.from.entropy_kj_per_kg_k,
        seg.to.entropy_kj_per_kg_k,
        n,
      );
      curveStates = statesFromPs({
        ...base,
        pressures_mpa: pressures,
        entropies_kj_per_kg_k: entropies,
      });
    }

    // Label the first point of each segment; skip the last to avoid
    // duplicates (the next segment's first point is the same state).
    for (let i = 0; i < curveStates.length - 1; i++) {
      const pt = curveStates[i];
      if (i === 0) pt.label = seg.label_from;
      allPoints.push(pt);
    }
  }

  return allPoints;
}

/**
 * Extract chart-specific (x, y) pairs from curve points.
 */
function toChartPoints(curvePoints, xKey, yKey) {
  return curvePoints.map(p => ({
    x: p[xKey],
    y: p[yKey],
    label: p.label,
  }));
}

function renderRecupProfile(states, model, fluid) {
  const [, s2, s3, , s5, s6] = states;
  const n = N_CURVE_POINTS;
  const base = { model, fluid };

  try {
    // Cold side (s2→s3): left to right.
    const coldStates = statesFromPh({
      ...base,
      pressures_mpa: linspace(s2.pressure_mpa, s3.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s2.enthalpy_kj_per_kg, s3.enthalpy_kj_per_kg, n),
    });

    // Hot side (s5→s6): counterflow, so hot inlet (s5) is at the cold
    // outlet end (right). Reverse so position 0 = cold inlet end.
    const hotStates = statesFromPh({
      ...base,
      pressures_mpa: linspace(s5.pressure_mpa, s6.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s5.enthalpy_kj_per_kg, s6.enthalpy_kj_per_kg, n),
    }).reverse();

    const coldTemps = coldStates.map(s => s.temperature_c);
    const hotTemps = hotStates.map(s => s.temperature_c);

    if (!recupChart) {
      recupChart = createRecupChart(document.getElementById('chart-recup'));
    }
    recupChart.update(hotTemps, coldTemps);
  } catch {
    // Silently skip if profile generation fails.
  }
}

function renderCharts(states, model, fluid) {
  let curvePoints;
  try {
    curvePoints = buildCurvePoints(states, model, fluid);
  } catch {
    // Fall back to straight lines between state points if curve
    // generation fails (e.g., thermo model can't evaluate a point).
    curvePoints = states.map((s, i) => ({ ...s, label: String(i + 1) }));
  }

  const tsPoints = toChartPoints(curvePoints, 'entropy_kj_per_kg_k', 'temperature_c');
  const hsPoints = toChartPoints(curvePoints, 'entropy_kj_per_kg_k', 'enthalpy_kj_per_kg');
  const pvPoints = curvePoints.map(p => ({
    x: 1 / p.density_kg_per_m3,
    y: p.pressure_mpa,
    label: p.label,
  }));
  const phPoints = toChartPoints(curvePoints, 'enthalpy_kj_per_kg', 'pressure_mpa');

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
    renderRecupProfile(result.states, input.model, input.fluid);
    renderCharts(result.states, input.model, input.fluid);
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

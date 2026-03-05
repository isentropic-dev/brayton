import { init, designPoint, recompDesignPoint, statesFromPh, statesFromPs } from './brayton.js';
import { createCycleChart } from './cycle-chart.js';
import { createRecupChart } from './recup-chart.js';

// ── Shared utilities ────────────────────────────────────────────────────────

function fmt(v, decimals = 2) {
  return v.toFixed(decimals);
}

const N_CURVE_POINTS = 30;

function linspace(a, b, n) {
  const arr = new Array(n);
  for (let i = 0; i < n; i++) {
    arr[i] = a + (b - a) * i / (n - 1);
  }
  return arr;
}

function toChartPoints(curvePoints, xKey, yKey) {
  return curvePoints.map(p => ({
    x: p[xKey],
    y: p[yKey],
    label: p.label,
  }));
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

function populateFluidSelect(selectEl, model) {
  const fluids = model === 'CoolProp' ? COOLPROP_FLUIDS : PERFECT_GAS_FLUIDS;
  const prev = selectEl.value;
  selectEl.innerHTML = '';
  for (const f of fluids) {
    const opt = document.createElement('option');
    opt.value = f.value;
    opt.textContent = f.label;
    selectEl.appendChild(opt);
  }
  if (fluids.some(f => f.value === prev)) {
    selectEl.value = prev;
  }
}

// ── Simple cycle ────────────────────────────────────────────────────────────

const S_STATE_LABELS = [
  'Compressor in',
  'Compressor out',
  'Recup cold out',
  'Turbine in',
  'Turbine out',
  'Recup hot out',
];

const S_INPUT_IDS = [
  's-compressor_inlet_temp_c',
  's-turbine_inlet_temp_c',
  's-compressor_inlet_pressure_mpa',
  's-compressor_outlet_pressure_mpa',
  's-net_power_mw',
  's-compressor_efficiency_pct',
  's-turbine_efficiency_pct',
  's-recuperator_ua_kw_per_k',
  's-recuperator_segments',
  's-recuperator_dp_cold_pct',
  's-recuperator_dp_hot_pct',
  's-precooler_dp_pct',
  's-primary_hx_dp_pct',
];

const S_PCT_FIELDS = [
  's-compressor_efficiency_pct',
  's-turbine_efficiency_pct',
  's-recuperator_dp_cold_pct',
  's-recuperator_dp_hot_pct',
  's-precooler_dp_pct',
  's-primary_hx_dp_pct',
];

// Map from UI IDs (with s- prefix) to facade field names.
const S_FIELD_MAP = {
  's-compressor_efficiency_pct': 'compressor_efficiency',
  's-turbine_efficiency_pct': 'turbine_efficiency',
  's-recuperator_dp_cold_pct': 'recuperator_dp_cold_fraction',
  's-recuperator_dp_hot_pct': 'recuperator_dp_hot_fraction',
  's-precooler_dp_pct': 'precooler_dp_fraction',
  's-primary_hx_dp_pct': 'primary_hx_dp_fraction',
};

function readSimpleInputs() {
  const obj = {};
  obj.model = document.getElementById('s-model').value;
  obj.fluid = document.getElementById('s-fluid').value;
  for (const id of S_INPUT_IDS) {
    const el = document.getElementById(id);
    const val = id === 's-recuperator_segments'
      ? parseInt(el.value, 10)
      : parseFloat(el.value);
    if (isNaN(val)) return null;
    // Strip s- prefix, then check map.
    const bare = id.replace(/^s-/, '');
    const key = S_FIELD_MAP[id] || bare;
    obj[key] = S_PCT_FIELDS.includes(id) ? val / 100 : val;
  }
  return obj;
}

let stsChart = null, shsChart = null, spvChart = null, sphChart = null, sRecupChart = null;

function renderSimpleScalars(r) {
  document.getElementById('s-r-eta').textContent = fmt(r.thermal_efficiency * 100, 2);
  document.getElementById('s-r-mass-flow').textContent = fmt(r.mass_flow_kg_per_s, 1);
  document.getElementById('s-r-comp-power').textContent = fmt(r.compressor_power_mw, 2);
  document.getElementById('s-r-turb-power').textContent = fmt(r.turbine_power_mw, 2);
  document.getElementById('s-r-heat-in').textContent = fmt(r.heat_input_mw, 2);
  document.getElementById('s-r-heat-rej').textContent = fmt(r.heat_rejection_mw, 2);
  document.getElementById('s-r-recup-q').textContent = fmt(r.recuperator_heat_transfer_mw, 2);
  document.getElementById('s-r-recup-min-dt').textContent = fmt(r.recuperator_min_delta_t_k, 1);
  document.getElementById('s-r-recup-effectiveness').textContent = fmt(r.recuperator_effectiveness, 3);
}

function renderSimpleStates(states) {
  const tbody = document.getElementById('s-state-tbody');
  tbody.innerHTML = '';
  states.forEach((s, i) => {
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td>${i + 1}</td>
      <td>${S_STATE_LABELS[i]}</td>
      <td>${fmt(s.temperature_c, 1)}</td>
      <td>${fmt(s.pressure_mpa, 2)}</td>
      <td>${fmt(s.density_kg_per_m3, 1)}</td>
      <td>${fmt(s.enthalpy_kj_per_kg, 1)}</td>
      <td>${fmt(s.entropy_kj_per_kg_k, 4)}</td>
    `;
    tbody.appendChild(tr);
  });
}

function buildSimpleCurvePoints(states, model, fluid) {
  const [s1, s2, s3, s4, s5, s6] = states;
  return buildCurveFromSegments([
    { from: s1, to: s2, method: 'ps', label_from: '1' },
    { from: s2, to: s3, method: 'ph', label_from: '2' },
    { from: s3, to: s4, method: 'ph', label_from: '3' },
    { from: s4, to: s5, method: 'ps', label_from: '4' },
    { from: s5, to: s6, method: 'ph', label_from: '5' },
    { from: s6, to: s1, method: 'ph', label_from: '6' },
  ], model, fluid);
}

function renderSimpleRecupProfile(states, model, fluid) {
  const [, s2, s3, , s5, s6] = states;
  const n = N_CURVE_POINTS;
  const base = { model, fluid };
  try {
    const coldStates = statesFromPh({
      ...base,
      pressures_mpa: linspace(s2.pressure_mpa, s3.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s2.enthalpy_kj_per_kg, s3.enthalpy_kj_per_kg, n),
    });
    const hotStates = statesFromPh({
      ...base,
      pressures_mpa: linspace(s5.pressure_mpa, s6.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s5.enthalpy_kj_per_kg, s6.enthalpy_kj_per_kg, n),
    }).reverse();
    if (!sRecupChart) {
      sRecupChart = createRecupChart(document.getElementById('s-chart-recup'));
    }
    sRecupChart.update(hotStates.map(s => s.temperature_c), coldStates.map(s => s.temperature_c));
  } catch { /* skip */ }
}

function renderSimpleCharts(states, model, fluid) {
  let curvePoints;
  try {
    curvePoints = buildSimpleCurvePoints(states, model, fluid);
  } catch {
    curvePoints = states.map((s, i) => ({ ...s, label: String(i + 1) }));
  }

  const tsPoints = toChartPoints(curvePoints, 'entropy_kj_per_kg_k', 'temperature_c');
  const hsPoints = toChartPoints(curvePoints, 'entropy_kj_per_kg_k', 'enthalpy_kj_per_kg');
  const pvPoints = curvePoints.map(p => ({ x: 1 / p.density_kg_per_m3, y: p.pressure_mpa, label: p.label }));
  const phPoints = toChartPoints(curvePoints, 'enthalpy_kj_per_kg', 'pressure_mpa');

  if (!stsChart) stsChart = createCycleChart(document.getElementById('s-chart-ts'), { title: 'T–s', xLabel: 's (kJ/kg·K)', yLabel: 'T (°C)' });
  stsChart.update(tsPoints);
  if (!shsChart) shsChart = createCycleChart(document.getElementById('s-chart-hs'), { title: 'h–s', xLabel: 's (kJ/kg·K)', yLabel: 'h (kJ/kg)' });
  shsChart.update(hsPoints);
  if (!spvChart) spvChart = createCycleChart(document.getElementById('s-chart-pv'), { title: 'P–v', xLabel: 'v (m³/kg)', yLabel: 'P (MPa)' });
  spvChart.update(pvPoints);
  if (!sphChart) sphChart = createCycleChart(document.getElementById('s-chart-ph'), { title: 'P–h', xLabel: 'h (kJ/kg)', yLabel: 'P (MPa)' });
  sphChart.update(phPoints);
}

function showSimpleError(msg) {
  document.getElementById('s-error').textContent = msg;
  document.getElementById('s-error').hidden = false;
  document.getElementById('s-results-content').style.opacity = '0.3';
}

function clearSimpleError() {
  document.getElementById('s-error').hidden = true;
  document.getElementById('s-results-content').style.opacity = '1';
}

function calculateSimple() {
  const input = readSimpleInputs();
  if (!input) { showSimpleError('Some inputs are empty or invalid.'); return; }
  try {
    const result = designPoint(input);
    clearSimpleError();
    renderSimpleScalars(result);
    renderSimpleStates(result.states);
    renderSimpleRecupProfile(result.states, input.model, input.fluid);
    renderSimpleCharts(result.states, input.model, input.fluid);
  } catch (e) {
    showSimpleError(e.message || String(e));
  }
}

// ── Recompression cycle ─────────────────────────────────────────────────────

const R_STATE_LABELS = [
  'Main compressor inlet',
  'Main compressor outlet',
  'Low temp recuperator cold outlet',
  'Mixing valve outlet',
  'High temp recuperator cold outlet',
  'Turbine inlet',
  'Turbine outlet',
  'High temp recuperator hot outlet',
  'Low temp recuperator hot outlet',
  'Recompressor outlet',
];

const R_INPUT_IDS = [
  'r-compressor_inlet_temp_c',
  'r-turbine_inlet_temp_c',
  'r-compressor_inlet_pressure_mpa',
  'r-compressor_outlet_pressure_mpa',
  'r-net_power_mw',
  'r-recomp_frac',
  'r-mc_efficiency_pct',
  'r-rc_efficiency_pct',
  'r-turbine_efficiency_pct',
  'r-lt_recuperator_ua_kw_per_k',
  'r-lt_recuperator_segments',
  'r-lt_recuperator_dp_cold_pct',
  'r-lt_recuperator_dp_hot_pct',
  'r-ht_recuperator_ua_kw_per_k',
  'r-ht_recuperator_segments',
  'r-ht_recuperator_dp_cold_pct',
  'r-ht_recuperator_dp_hot_pct',
  'r-precooler_dp_pct',
  'r-primary_hx_dp_pct',
];

const R_PCT_FIELDS = [
  'r-mc_efficiency_pct',
  'r-rc_efficiency_pct',
  'r-turbine_efficiency_pct',
  'r-lt_recuperator_dp_cold_pct',
  'r-lt_recuperator_dp_hot_pct',
  'r-ht_recuperator_dp_cold_pct',
  'r-ht_recuperator_dp_hot_pct',
  'r-precooler_dp_pct',
  'r-primary_hx_dp_pct',
];

const R_FIELD_MAP = {
  'r-mc_efficiency_pct': 'mc_efficiency',
  'r-rc_efficiency_pct': 'rc_efficiency',
  'r-turbine_efficiency_pct': 'turbine_efficiency',
  'r-lt_recuperator_dp_cold_pct': 'lt_recuperator_dp_cold_fraction',
  'r-lt_recuperator_dp_hot_pct': 'lt_recuperator_dp_hot_fraction',
  'r-ht_recuperator_dp_cold_pct': 'ht_recuperator_dp_cold_fraction',
  'r-ht_recuperator_dp_hot_pct': 'ht_recuperator_dp_hot_fraction',
  'r-precooler_dp_pct': 'precooler_dp_fraction',
  'r-primary_hx_dp_pct': 'primary_hx_dp_fraction',
};

const R_SEGMENT_FIELDS = [
  'r-lt_recuperator_segments',
  'r-ht_recuperator_segments',
];

function readRecompInputs() {
  const obj = {};
  obj.model = document.getElementById('r-model').value;
  obj.fluid = document.getElementById('r-fluid').value;
  for (const id of R_INPUT_IDS) {
    const el = document.getElementById(id);
    const val = R_SEGMENT_FIELDS.includes(id)
      ? parseInt(el.value, 10)
      : parseFloat(el.value);
    if (isNaN(val)) return null;
    const bare = id.replace(/^r-/, '');
    const key = R_FIELD_MAP[id] || bare;
    obj[key] = R_PCT_FIELDS.includes(id) ? val / 100 : val;
  }
  return obj;
}

let rtsChart = null, rhsChart = null, rpvChart = null, rphChart = null;
let rLtRecupChart = null, rHtRecupChart = null;

function renderRecompScalars(r) {
  document.getElementById('r-r-eta').textContent = fmt(r.thermal_efficiency * 100, 2);
  document.getElementById('r-r-mass-flow-total').textContent = fmt(r.mass_flow_total_kg_per_s, 1);
  document.getElementById('r-r-mass-flow-mc').textContent = fmt(r.mass_flow_mc_kg_per_s, 1);
  document.getElementById('r-r-mass-flow-rc').textContent = fmt(r.mass_flow_rc_kg_per_s, 1);
  document.getElementById('r-r-mc-power').textContent = fmt(r.mc_power_mw, 2);
  document.getElementById('r-r-rc-power').textContent = fmt(r.rc_power_mw, 2);
  document.getElementById('r-r-turb-power').textContent = fmt(r.turbine_power_mw, 2);
  document.getElementById('r-r-heat-in').textContent = fmt(r.heat_input_mw, 2);
  document.getElementById('r-r-heat-rej').textContent = fmt(r.heat_rejection_mw, 2);
  document.getElementById('r-r-lt-q').textContent = fmt(r.lt_recuperator_heat_transfer_mw, 2);
  document.getElementById('r-r-lt-min-dt').textContent = fmt(r.lt_recuperator_min_delta_t_k, 1);
  document.getElementById('r-r-lt-effectiveness').textContent = fmt(r.lt_recuperator_effectiveness, 3);
  document.getElementById('r-r-ht-q').textContent = fmt(r.ht_recuperator_heat_transfer_mw, 2);
  document.getElementById('r-r-ht-min-dt').textContent = fmt(r.ht_recuperator_min_delta_t_k, 1);
  document.getElementById('r-r-ht-effectiveness').textContent = fmt(r.ht_recuperator_effectiveness, 3);
}

function renderRecompStates(states) {
  const tbody = document.getElementById('r-state-tbody');
  tbody.innerHTML = '';
  states.forEach((s, i) => {
    const tr = document.createElement('tr');
    tr.innerHTML = `
      <td>${i + 1}</td>
      <td>${R_STATE_LABELS[i]}</td>
      <td>${fmt(s.temperature_c, 1)}</td>
      <td>${fmt(s.pressure_mpa, 2)}</td>
      <td>${fmt(s.density_kg_per_m3, 1)}</td>
      <td>${fmt(s.enthalpy_kj_per_kg, 1)}</td>
      <td>${fmt(s.entropy_kj_per_kg_k, 4)}</td>
    `;
    tbody.appendChild(tr);
  });
}

function renderRecompRecupProfiles(states, model, fluid) {
  const n = N_CURVE_POINTS;
  const base = { model, fluid };
  // States: s1..s10 (indices 0..9).
  const [, s2, s3, , s5, , s7, s8, s9] = states;

  // LT recuperator: cold = s2→s3, hot = s8→s9.
  // Counterflow: hot inlet (s8) at cold outlet end. Reverse hot for position alignment.
  try {
    const ltCold = statesFromPh({
      ...base,
      pressures_mpa: linspace(s2.pressure_mpa, s3.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s2.enthalpy_kj_per_kg, s3.enthalpy_kj_per_kg, n),
    });
    const ltHot = statesFromPh({
      ...base,
      pressures_mpa: linspace(s8.pressure_mpa, s9.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s8.enthalpy_kj_per_kg, s9.enthalpy_kj_per_kg, n),
    }).reverse();
    if (!rLtRecupChart) {
      rLtRecupChart = createRecupChart(document.getElementById('r-chart-lt-recup'));
    }
    rLtRecupChart.update(ltHot.map(s => s.temperature_c), ltCold.map(s => s.temperature_c));
  } catch { /* skip */ }

  // HT recuperator: cold = s4→s5 (index 3→4), hot = s7→s8 (index 6→7).
  // Use s5 for cold out (index 4) since s4 is the mixing valve outlet.
  const s4 = states[3];
  try {
    const htCold = statesFromPh({
      ...base,
      pressures_mpa: linspace(s4.pressure_mpa, s5.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s4.enthalpy_kj_per_kg, s5.enthalpy_kj_per_kg, n),
    });
    const htHot = statesFromPh({
      ...base,
      pressures_mpa: linspace(s7.pressure_mpa, s8.pressure_mpa, n),
      enthalpies_kj_per_kg: linspace(s7.enthalpy_kj_per_kg, s8.enthalpy_kj_per_kg, n),
    }).reverse();
    if (!rHtRecupChart) {
      rHtRecupChart = createRecupChart(document.getElementById('r-chart-ht-recup'));
    }
    rHtRecupChart.update(htHot.map(s => s.temperature_c), htCold.map(s => s.temperature_c));
  } catch { /* skip */ }
}

function buildCurveFromSegments(segments, model, fluid) {
  const n = N_CURVE_POINTS;
  const base = { model, fluid };
  const allPoints = [];
  for (const seg of segments) {
    const pressures = linspace(seg.from.pressure_mpa, seg.to.pressure_mpa, n);
    let curveStates;
    if (seg.method === 'ph') {
      curveStates = statesFromPh({
        ...base,
        pressures_mpa: pressures,
        enthalpies_kj_per_kg: linspace(seg.from.enthalpy_kj_per_kg, seg.to.enthalpy_kj_per_kg, n),
      });
    } else {
      curveStates = statesFromPs({
        ...base,
        pressures_mpa: pressures,
        entropies_kj_per_kg_k: linspace(seg.from.entropy_kj_per_kg_k, seg.to.entropy_kj_per_kg_k, n),
      });
    }
    for (let i = 0; i < curveStates.length - 1; i++) {
      const pt = curveStates[i];
      if (i === 0) pt.label = seg.label_from;
      allPoints.push(pt);
    }
  }
  return allPoints;
}

function buildRecompCurvePoints(states, model, fluid) {
  const [s1, s2, s3, s4, s5, s6, s7, s8, s9, s10] = states;

  // Main flow path: 1→2→3→4→5→6→7→8→9→1.
  const mainSegments = [
    { from: s1, to: s2, method: 'ps', label_from: '1' },
    { from: s2, to: s3, method: 'ph', label_from: '2' },
    { from: s3, to: s4, method: 'ph', label_from: '3' },
    { from: s4, to: s5, method: 'ph', label_from: '4' },
    { from: s5, to: s6, method: 'ph', label_from: '5' },
    { from: s6, to: s7, method: 'ps', label_from: '6' },
    { from: s7, to: s8, method: 'ph', label_from: '7' },
    { from: s8, to: s9, method: 'ph', label_from: '8' },
    { from: s9, to: s1, method: 'ph', label_from: '9' },
  ];

  // Recompressor branch: 9→10→4.
  const branchSegments = [
    { from: s9, to: s10, method: 'ps' },
    { from: s10, to: s4, method: 'ph', label_from: '10' },
  ];

  const mainPoints = buildCurveFromSegments(mainSegments, model, fluid);
  const branchPoints = buildCurveFromSegments(branchSegments, model, fluid);

  return { mainPoints, branchPoints };
}

function renderRecompCharts(states, model, fluid) {
  let mainPoints, branchPoints;
  try {
    ({ mainPoints, branchPoints } = buildRecompCurvePoints(states, model, fluid));
  } catch {
    // Fallback: straight lines between state points.
    const mainPath = [0, 1, 2, 3, 4, 5, 6, 7, 8];
    mainPoints = mainPath.map(i => ({ ...states[i], label: String(i + 1) }));
    branchPoints = [
      { ...states[9], label: '10' },
      { ...states[3] },
    ];
  }

  function toBranches(points, xKey, yKey) {
    return [toChartPoints(points, xKey, yKey)];
  }
  function toBranchesPv(points) {
    return [points.map(p => ({ x: 1 / p.density_kg_per_m3, y: p.pressure_mpa, label: p.label }))];
  }

  const tsPoints = toChartPoints(mainPoints, 'entropy_kj_per_kg_k', 'temperature_c');
  const tsBranches = toBranches(branchPoints, 'entropy_kj_per_kg_k', 'temperature_c');
  const hsPoints = toChartPoints(mainPoints, 'entropy_kj_per_kg_k', 'enthalpy_kj_per_kg');
  const hsBranches = toBranches(branchPoints, 'entropy_kj_per_kg_k', 'enthalpy_kj_per_kg');
  const pvPoints = mainPoints.map(p => ({ x: 1 / p.density_kg_per_m3, y: p.pressure_mpa, label: p.label }));
  const pvBranches = toBranchesPv(branchPoints);
  const phPoints = toChartPoints(mainPoints, 'enthalpy_kj_per_kg', 'pressure_mpa');
  const phBranches = toBranches(branchPoints, 'enthalpy_kj_per_kg', 'pressure_mpa');

  if (!rtsChart) rtsChart = createCycleChart(document.getElementById('r-chart-ts'), { title: 'T–s', xLabel: 's (kJ/kg·K)', yLabel: 'T (°C)' });
  rtsChart.update(tsPoints, { branches: tsBranches });
  if (!rhsChart) rhsChart = createCycleChart(document.getElementById('r-chart-hs'), { title: 'h–s', xLabel: 's (kJ/kg·K)', yLabel: 'h (kJ/kg)' });
  rhsChart.update(hsPoints, { branches: hsBranches });
  if (!rpvChart) rpvChart = createCycleChart(document.getElementById('r-chart-pv'), { title: 'P–v', xLabel: 'v (m³/kg)', yLabel: 'P (MPa)' });
  rpvChart.update(pvPoints, { branches: pvBranches });
  if (!rphChart) rphChart = createCycleChart(document.getElementById('r-chart-ph'), { title: 'P–h', xLabel: 'h (kJ/kg)', yLabel: 'P (MPa)' });
  rphChart.update(phPoints, { branches: phBranches });
}

function showRecompError(msg) {
  document.getElementById('r-error').textContent = msg;
  document.getElementById('r-error').hidden = false;
  document.getElementById('r-results-content').style.opacity = '0.3';
}

function clearRecompError() {
  document.getElementById('r-error').hidden = true;
  document.getElementById('r-results-content').style.opacity = '1';
}

function calculateRecomp() {
  const input = readRecompInputs();
  if (!input) { showRecompError('Some inputs are empty or invalid.'); return; }
  try {
    const result = recompDesignPoint(input);
    clearRecompError();
    renderRecompScalars(result);
    renderRecompStates(result.states);
    renderRecompRecupProfiles(result.states, input.model, input.fluid);
    renderRecompCharts(result.states, input.model, input.fluid);
  } catch (e) {
    showRecompError(e.message || String(e));
  }
}

// ── Tab switching ───────────────────────────────────────────────────────────

let activeCycle = 'simple';
let simpleNeedsCalc = true;
let recompNeedsCalc = true;

function switchCycle(cycle) {
  if (cycle === activeCycle) return;

  const oldView = document.getElementById(
    activeCycle === 'simple' ? 'simple-cycle' : 'recompression-cycle'
  );
  const newView = document.getElementById(
    cycle === 'simple' ? 'simple-cycle' : 'recompression-cycle'
  );

  activeCycle = cycle;

  document.querySelectorAll('.tab').forEach(t => {
    t.classList.toggle('active', t.dataset.cycle === cycle);
  });

  oldView.classList.add('fading-out');
  function swap() {
    oldView.hidden = true;
    oldView.classList.remove('fading-out');
    newView.hidden = false;
  }
  oldView.addEventListener('transitionend', swap, { once: true });
  // Fallback if transitionend doesn't fire (e.g., rapid clicks, skipped transition).
  setTimeout(swap, 350);

  // Calculate on first switch if needed.
  if (cycle === 'simple' && simpleNeedsCalc) {
    simpleNeedsCalc = false;
    calculateSimple();
  } else if (cycle === 'recompression' && recompNeedsCalc) {
    recompNeedsCalc = false;
    calculateRecomp();
  }
}

// ── Debounced input handling ────────────────────────────────────────────────

let sDebounce = null;
let rDebounce = null;

function onSimpleInput() {
  clearTimeout(sDebounce);
  sDebounce = setTimeout(calculateSimple, 300);
}

function onRecompInput() {
  clearTimeout(rDebounce);
  rDebounce = setTimeout(calculateRecomp, 300);
}

// ── Initialization ──────────────────────────────────────────────────────────

function nextFrame() {
  return new Promise(resolve => requestAnimationFrame(() => setTimeout(resolve, 0)));
}

async function main() {
  const overlay = document.getElementById('loading-overlay');

  await init();

  // Tab buttons.
  document.querySelectorAll('.tab').forEach(t => {
    t.addEventListener('click', () => switchCycle(t.dataset.cycle));
  });

  // Simple cycle inputs.
  for (const id of S_INPUT_IDS) {
    document.getElementById(id).addEventListener('input', onSimpleInput);
  }
  const sModelEl = document.getElementById('s-model');
  const sFluidEl = document.getElementById('s-fluid');
  sModelEl.addEventListener('change', () => {
    populateFluidSelect(sFluidEl, sModelEl.value);
    onSimpleInput();
  });
  sFluidEl.addEventListener('change', onSimpleInput);
  populateFluidSelect(sFluidEl, sModelEl.value);

  // Recompression cycle inputs.
  for (const id of R_INPUT_IDS) {
    document.getElementById(id).addEventListener('input', onRecompInput);
  }
  const rModelEl = document.getElementById('r-model');
  const rFluidEl = document.getElementById('r-fluid');
  rModelEl.addEventListener('change', () => {
    populateFluidSelect(rFluidEl, rModelEl.value);
    onRecompInput();
  });
  rFluidEl.addEventListener('change', onRecompInput);
  populateFluidSelect(rFluidEl, rModelEl.value);

  // Yield so the loading overlay renders before the first (slow) calculations.
  await nextFrame();

  // Calculate both tabs on load so switching is instant.
  simpleNeedsCalc = false;
  recompNeedsCalc = false;
  calculateSimple();
  calculateRecomp();

  overlay.hidden = true;
}

main().catch(e => {
  const el = document.getElementById('s-error') || document.getElementById('r-error');
  if (el) {
    el.textContent = 'Failed to initialize WASM: ' + (e.message || e);
    el.hidden = false;
  }
});

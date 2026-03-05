/**
 * Lightweight cycle diagram renderer using Canvas 2D.
 *
 * API:
 *   const chart = createCycleChart(container, { title, xLabel, yLabel })
 *   chart.update(points, { branches })
 *     points = [{ x, y, label }, ...]  — main cycle (closed loop)
 *     branches = [[{ x, y, label }, ...], ...]  — optional open paths
 *
 * To swap rendering (e.g., to a charting library), replace this file
 * and keep the same create/update interface.
 */

const CHART_WIDTH = 576;
const CHART_HEIGHT = 336;
const PADDING = { top: 40, right: 30, bottom: 50, left: 65 };
const POINT_RADIUS = 9;
const COLORS = {
  line: '#2563eb',
  fill: 'rgba(37, 99, 235, 0.06)',
  point: '#2563eb',
  label: '#1a1a1a',
  axis: '#888',
  grid: '#e5e5e5',
  title: '#333',
};

export function createCycleChart(container, { title, xLabel, yLabel }) {
  const canvas = document.createElement('canvas');
  const dpr = window.devicePixelRatio || 1;
  container.appendChild(canvas);

  const ctx = canvas.getContext('2d');
  let currentPoints = null;


  function niceRange(min, max) {
    if (min === max) {
      min -= 1;
      max += 1;
    }
    const range = max - min;
    const margin = range * 0.1;
    return [min - margin, max + margin];
  }

  function niceStep(range) {
    const rough = range / 5;
    const mag = Math.pow(10, Math.floor(Math.log10(rough)));
    const norm = rough / mag;
    let step;
    if (norm <= 1.5) step = 1;
    else if (norm <= 3.5) step = 2;
    else if (norm <= 7.5) step = 5;
    else step = 10;
    return step * mag;
  }

  function draw(points, branches, w, h) {
    ctx.clearRect(0, 0, w, h);

    const plotW = w - PADDING.left - PADDING.right;
    const plotH = h - PADDING.top - PADDING.bottom;

    // Compute ranges from all data (main + branches).
    const allPoints = points.concat(...branches);
    const xs = allPoints.map(p => p.x);
    const ys = allPoints.map(p => p.y);
    const [xMin, xMax] = niceRange(Math.min(...xs), Math.max(...xs));
    const [yMin, yMax] = niceRange(Math.min(...ys), Math.max(...ys));

    function toCanvasX(v) { return PADDING.left + ((v - xMin) / (xMax - xMin)) * plotW; }
    function toCanvasY(v) { return PADDING.top + plotH - ((v - yMin) / (yMax - yMin)) * plotH; }

    // Grid and axes.
    ctx.strokeStyle = COLORS.grid;
    ctx.lineWidth = 0.5;
    ctx.fillStyle = COLORS.axis;
    ctx.font = '11px -apple-system, sans-serif';
    ctx.textAlign = 'center';

    const xStep = niceStep(xMax - xMin);
    const xStart = Math.ceil(xMin / xStep) * xStep;
    for (let v = xStart; v <= xMax; v += xStep) {
      const cx = toCanvasX(v);
      ctx.beginPath();
      ctx.moveTo(cx, PADDING.top);
      ctx.lineTo(cx, PADDING.top + plotH);
      ctx.stroke();
      ctx.fillText(formatTick(v), cx, PADDING.top + plotH + 16);
    }

    ctx.textAlign = 'right';
    const yStep = niceStep(yMax - yMin);
    const yStart = Math.ceil(yMin / yStep) * yStep;
    for (let v = yStart; v <= yMax; v += yStep) {
      const cy = toCanvasY(v);
      ctx.beginPath();
      ctx.moveTo(PADDING.left, cy);
      ctx.lineTo(PADDING.left + plotW, cy);
      ctx.stroke();
      ctx.fillText(formatTick(v), PADDING.left - 8, cy + 4);
    }

    // Plot border.
    ctx.strokeStyle = COLORS.axis;
    ctx.lineWidth = 1;
    ctx.strokeRect(PADDING.left, PADDING.top, plotW, plotH);

    // Filled cycle polygon.
    ctx.beginPath();
    ctx.moveTo(toCanvasX(points[0].x), toCanvasY(points[0].y));
    for (let i = 1; i < points.length; i++) {
      ctx.lineTo(toCanvasX(points[i].x), toCanvasY(points[i].y));
    }
    ctx.closePath();
    ctx.fillStyle = COLORS.fill;
    ctx.fill();

    // Cycle line.
    ctx.beginPath();
    ctx.moveTo(toCanvasX(points[0].x), toCanvasY(points[0].y));
    for (let i = 1; i < points.length; i++) {
      ctx.lineTo(toCanvasX(points[i].x), toCanvasY(points[i].y));
    }
    ctx.closePath();
    ctx.strokeStyle = COLORS.line;
    ctx.lineWidth = 2;
    ctx.stroke();

    // Branch lines (open paths, dashed).
    for (const branch of branches) {
      if (branch.length < 2) continue;
      ctx.beginPath();
      ctx.moveTo(toCanvasX(branch[0].x), toCanvasY(branch[0].y));
      for (let i = 1; i < branch.length; i++) {
        ctx.lineTo(toCanvasX(branch[i].x), toCanvasY(branch[i].y));
      }
      ctx.strokeStyle = COLORS.line;
      ctx.lineWidth = 2;
      ctx.setLineDash([6, 4]);
      ctx.stroke();
      ctx.setLineDash([]);
    }

    // State points: hollow circles with number inside.
    const labeled = allPoints.filter(p => p.label);
    for (const p of labeled) {
      const cx = toCanvasX(p.x);
      const cy = toCanvasY(p.y);

      // White fill to clear the cycle line behind the circle.
      ctx.beginPath();
      ctx.arc(cx, cy, POINT_RADIUS, 0, Math.PI * 2);
      ctx.fillStyle = '#fff';
      ctx.fill();
      ctx.strokeStyle = COLORS.point;
      ctx.lineWidth = 1.5;
      ctx.stroke();

      ctx.fillStyle = COLORS.label;
      ctx.font = 'bold 9px -apple-system, sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(p.label, cx, cy);
    }

    // Title.
    ctx.fillStyle = COLORS.title;
    ctx.font = 'bold 13px -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(title, w / 2, 20);

    // Axis labels.
    ctx.fillStyle = COLORS.axis;
    ctx.font = '12px -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText(xLabel, PADDING.left + plotW / 2, h - 6);

    ctx.save();
    ctx.translate(14, PADDING.top + plotH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText(yLabel, 0, 0);
    ctx.restore();
  }

  function formatTick(v) {
    if (Math.abs(v) >= 1000) return v.toFixed(0);
    if (Math.abs(v) >= 1) return v.toFixed(1);
    return v.toFixed(3);
  }

  let lastPoints = null, lastBranches = [];

  function render() {
    if (!lastPoints) return;
    const w = Math.min(CHART_WIDTH, container.clientWidth || CHART_WIDTH);
    const h = CHART_HEIGHT;
    canvas.style.width = w + 'px';
    canvas.style.height = h + 'px';
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(lastPoints, lastBranches, w, h);
  }

  function update(points, { branches = [] } = {}) {
    lastPoints = points;
    lastBranches = branches;
    render();
  }

  new ResizeObserver(render).observe(container);

  return { update };
}

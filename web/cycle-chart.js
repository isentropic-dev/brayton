/**
 * Lightweight cycle diagram renderer using Canvas 2D.
 *
 * API:
 *   const chart = createCycleChart(container, { title, xLabel, yLabel })
 *   chart.update(points)  // points = [{ x, y, label }, ...]
 *
 * Points are drawn connected in order, with the last point connecting
 * back to the first (closing the cycle). Each point gets a label.
 *
 * To swap rendering (e.g., to a charting library), replace this file
 * and keep the same create/update interface.
 */

const CHART_WIDTH = 360;
const CHART_HEIGHT = 280;
const PADDING = { top: 40, right: 30, bottom: 50, left: 65 };
const POINT_RADIUS = 4;
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

  function draw(points, w, h) {
    ctx.clearRect(0, 0, w, h);

    const plotW = w - PADDING.left - PADDING.right;
    const plotH = h - PADDING.top - PADDING.bottom;

    // Compute ranges from data.
    const xs = points.map(p => p.x);
    const ys = points.map(p => p.y);
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

    // Points and labels.
    for (const p of points) {
      const cx = toCanvasX(p.x);
      const cy = toCanvasY(p.y);

      ctx.beginPath();
      ctx.arc(cx, cy, POINT_RADIUS, 0, Math.PI * 2);
      ctx.fillStyle = COLORS.point;
      ctx.fill();

      if (p.label) {
        ctx.fillStyle = COLORS.label;
        ctx.font = 'bold 11px -apple-system, sans-serif';
        ctx.textAlign = 'left';
        ctx.fillText(p.label, cx + 7, cy - 7);
      }
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

  function update(points) {
    currentPoints = points;
    canvas.style.width = CHART_WIDTH + 'px';
    canvas.style.height = CHART_HEIGHT + 'px';
    canvas.width = CHART_WIDTH * dpr;
    canvas.height = CHART_HEIGHT * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(points, CHART_WIDTH, CHART_HEIGHT);
  }

  return { update };
}

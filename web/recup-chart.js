/**
 * Recuperator temperature profile chart.
 *
 * Draws hot and cold stream temperatures vs. position through the HX.
 *
 * API:
 *   const chart = createRecupChart(container)
 *   chart.update(hotTemps, coldTemps)  // arrays of temperature in °C
 */

const CHART_WIDTH = 740;
const CHART_HEIGHT = 280;
const PADDING = { top: 20, right: 30, bottom: 50, left: 65 };
const COLORS = {
  hot: '#dc2626',
  cold: '#2563eb',
  axis: '#888',
  grid: '#e5e5e5',
  title: '#333',
  label: '#555',
};

export function createRecupChart(container) {
  const canvas = document.createElement('canvas');
  const dpr = window.devicePixelRatio || 1;
  container.appendChild(canvas);

  const ctx = canvas.getContext('2d');

  function niceRange(min, max) {
    if (min === max) { min -= 1; max += 1; }
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

  function formatTick(v) {
    if (Math.abs(v) >= 1000) return v.toFixed(0);
    if (Math.abs(v) >= 1) return v.toFixed(1);
    return v.toFixed(3);
  }

  function draw(hotTemps, coldTemps, w, h) {
    ctx.clearRect(0, 0, w, h);

    const plotW = w - PADDING.left - PADDING.right;
    const plotH = h - PADDING.top - PADDING.bottom;

    const n = hotTemps.length;
    const allTemps = hotTemps.concat(coldTemps);
    const [yMin, yMax] = niceRange(Math.min(...allTemps), Math.max(...allTemps));

    function toCanvasX(i) { return PADDING.left + (i / (n - 1)) * plotW; }
    function toCanvasY(v) { return PADDING.top + plotH - ((v - yMin) / (yMax - yMin)) * plotH; }

    // Grid and y-axis labels.
    ctx.strokeStyle = COLORS.grid;
    ctx.lineWidth = 0.5;
    ctx.fillStyle = COLORS.axis;
    ctx.font = '11px -apple-system, sans-serif';

    const yStep = niceStep(yMax - yMin);
    const yStart = Math.ceil(yMin / yStep) * yStep;
    ctx.textAlign = 'right';
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

    // Draw hot stream.
    ctx.beginPath();
    ctx.moveTo(toCanvasX(0), toCanvasY(hotTemps[0]));
    for (let i = 1; i < n; i++) {
      ctx.lineTo(toCanvasX(i), toCanvasY(hotTemps[i]));
    }
    ctx.strokeStyle = COLORS.hot;
    ctx.lineWidth = 2;
    ctx.stroke();

    // Draw cold stream.
    ctx.beginPath();
    ctx.moveTo(toCanvasX(0), toCanvasY(coldTemps[0]));
    for (let i = 1; i < n; i++) {
      ctx.lineTo(toCanvasX(i), toCanvasY(coldTemps[i]));
    }
    ctx.strokeStyle = COLORS.cold;
    ctx.lineWidth = 2;
    ctx.stroke();

    // Axis labels.
    ctx.fillStyle = COLORS.axis;
    ctx.font = '12px -apple-system, sans-serif';
    ctx.textAlign = 'center';
    ctx.fillText('Position', PADDING.left + plotW / 2, h - 6);

    ctx.save();
    ctx.translate(14, PADDING.top + plotH / 2);
    ctx.rotate(-Math.PI / 2);
    ctx.fillText('T (°C)', 0, 0);
    ctx.restore();

    // Legend.
    const legendX = PADDING.left + 12;
    const legendY = PADDING.top + 16;
    ctx.font = '11px -apple-system, sans-serif';
    ctx.textAlign = 'left';

    ctx.strokeStyle = COLORS.hot;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.moveTo(legendX, legendY);
    ctx.lineTo(legendX + 20, legendY);
    ctx.stroke();
    ctx.fillStyle = COLORS.hot;
    ctx.fillText('Hot side', legendX + 25, legendY + 4);

    ctx.strokeStyle = COLORS.cold;
    ctx.beginPath();
    ctx.moveTo(legendX, legendY + 18);
    ctx.lineTo(legendX + 20, legendY + 18);
    ctx.stroke();
    ctx.fillStyle = COLORS.cold;
    ctx.fillText('Cold side', legendX + 25, legendY + 22);
  }

  function update(hotTemps, coldTemps) {
    canvas.style.width = CHART_WIDTH + 'px';
    canvas.style.height = CHART_HEIGHT + 'px';
    canvas.width = CHART_WIDTH * dpr;
    canvas.height = CHART_HEIGHT * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    draw(hotTemps, coldTemps, CHART_WIDTH, CHART_HEIGHT);
  }

  return { update };
}

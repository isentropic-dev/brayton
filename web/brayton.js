// Minimal loader for the Emscripten-compiled brayton WASM module.
//
// Provides the small set of runtime imports the module needs (WASI stubs,
// emscripten memory growth notification) and exposes a single async
// entry point.
//
// Usage:
//   import { init, designPoint } from './brayton.js';
//   await init();
//   const result = designPoint({ ... });

const encoder = new TextEncoder();
const decoder = new TextDecoder();

let wasm = null;

function writeString(str) {
  const bytes = encoder.encode(str + '\0');
  const ptr = wasm.malloc(bytes.length);
  new Uint8Array(wasm.memory.buffer).set(bytes, ptr);
  return ptr;
}

function readString(ptr) {
  const mem = new Uint8Array(wasm.memory.buffer);
  let end = ptr;
  while (mem[end] !== 0) end++;
  return decoder.decode(mem.slice(ptr, end));
}

function makeImports() {
  return {
    wasi_snapshot_preview1: {
      environ_sizes_get: (countPtr, sizePtr) => {
        const view = new DataView(wasm.memory.buffer);
        view.setUint32(countPtr, 0, true);
        view.setUint32(sizePtr, 0, true);
        return 0;
      },
      environ_get: () => 0,
      fd_close: () => 0,
      fd_read: (fd, iovPtr, iovLen, nreadPtr) => {
        // Signal EOF by reporting zero bytes read.
        const view = new DataView(wasm.memory.buffer);
        view.setUint32(nreadPtr, 0, true);
        return 0;
      },
      fd_seek: (fd, offsetLo, offsetHi, whence, newoffsetPtr) => {
        const view = new DataView(wasm.memory.buffer);
        view.setBigUint64(newoffsetPtr, 0n, true);
        return 0;
      },
      fd_write: (fd, iovPtr, iovLen, nwrittenPtr) => {
        // Route fd_write to console.warn for panic/debug messages.
        const view = new DataView(wasm.memory.buffer);
        const mem = new Uint8Array(wasm.memory.buffer);
        let written = 0;
        let text = '';
        for (let i = 0; i < iovLen; i++) {
          const ptr = view.getUint32(iovPtr + i * 8, true);
          const len = view.getUint32(iovPtr + i * 8 + 4, true);
          text += decoder.decode(mem.slice(ptr, ptr + len));
          written += len;
        }
        if (text.trim()) console.warn('[wasm]', text);
        view.setUint32(nwrittenPtr, written, true);
        return 0;
      },
      // Simplified: always returns monotonic time regardless of clockId.
      // CoolProp only uses this for internal timing, not wall-clock time.
      clock_time_get: (clockId, precision, timePtr) => {
        const view = new DataView(wasm.memory.buffer);
        const now = BigInt(Math.round(performance.now() * 1e6));
        view.setBigUint64(timePtr, now, true);
        return 0;
      },
    },
    env: {
      emscripten_notify_memory_growth: () => {},
      __syscall_getcwd: () => -1,
      __syscall_getdents64: () => -1,
    },
  };
}

/**
 * Initialize the WASM module. Must be called once before `designPoint`.
 * Fetches the .wasm file relative to this script's location.
 */
export async function init() {
  if (wasm) return;
  const url = new URL('brayton.wasm', import.meta.url);
  const imports = makeImports();

  // Use streaming instantiation when available (browser with proper MIME type).
  let instance;
  if (typeof WebAssembly.instantiateStreaming === 'function') {
    try {
      const result = await WebAssembly.instantiateStreaming(fetch(url), imports);
      instance = result.instance;
    } catch {
      // Fallback if streaming fails (e.g., wrong MIME type from dev server).
      const buf = await (await fetch(url)).arrayBuffer();
      const result = await WebAssembly.instantiate(buf, imports);
      instance = result.instance;
    }
  } else {
    const buf = await (await fetch(url)).arrayBuffer();
    const result = await WebAssembly.instantiate(buf, imports);
    instance = result.instance;
  }

  wasm = instance.exports;
  wasm._initialize();
}

/**
 * Call a WASM function that takes JSON input and returns JSON output.
 * Handles string marshalling and error extraction.
 */
function callWasm(fn_name, input) {
  if (!wasm) throw new Error('WASM not initialized — call init() first');
  if (!wasm[fn_name]) throw new Error(`WASM function "${fn_name}" not found`);

  const inputPtr = writeString(JSON.stringify(input));
  const resultPtr = wasm[fn_name](inputPtr);
  wasm.free(inputPtr);
  const json = readString(resultPtr);
  wasm.free_result(resultPtr);

  const result = JSON.parse(json);
  if (result.error) throw new Error(result.error);
  return result;
}

/**
 * Run a design-point calculation.
 * @param {Object} input — plain object matching DesignPointInput fields
 * @returns {Object} — DesignPointOutput fields
 * @throws {Error} on invalid input or solver failure
 */
export function designPoint(input) {
  return callWasm('design_point', input);
}

/**
 * Compute thermodynamic states from arrays of pressure and enthalpy.
 * @param {Object} input — { model, fluid, pressures_mpa: number[], enthalpies_kj_per_kg: number[] }
 * @returns {Object[]} — array of StatePoint objects
 * @throws {Error} on invalid input or thermo model failure
 */
export function statesFromPh(input) {
  return callWasm('states_from_ph', input);
}

/**
 * Compute thermodynamic states from arrays of pressure and entropy.
 * @param {Object} input — { model, fluid, pressures_mpa: number[], entropies_kj_per_kg_k: number[] }
 * @returns {Object[]} — array of StatePoint objects
 * @throws {Error} on invalid input or thermo model failure
 */
export function statesFromPs(input) {
  return callWasm('states_from_ps', input);
}

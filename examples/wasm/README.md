# Brayton WASM Smoke Test

A minimal HTML test page that calls the `design_point` function compiled to WebAssembly and logs the result.

## Prerequisites

- [`wasm-pack`](https://rustwasm.github.io/wasm-pack/installer/)
- The `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`

## Build

From the repo root:

```bash
wasm-pack build --target web --features wasm
```

Output goes to `pkg/`.

## Serve

```bash
cd examples/wasm
python3 -m http.server 8080
```

Open [http://localhost:8080](http://localhost:8080). The page calls `design_point` with baseline inputs and renders the result as JSON. Check the browser console for any errors.

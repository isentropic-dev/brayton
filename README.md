# Brayton Cycle Models for Supercritical CO₂

Brayton cycle models for sCO₂ power cycles, built on [Twine](https://github.com/isentropic-dev/twine-models).

**[Try the interactive dashboard →](https://isentropic-dev.github.io/brayton/)**

This project recreates the design-point and off-design sCO₂ cycle models from [this thesis](https://github.com/isentropic-dev/brayton/blob/main/docs/dyreby_thesis.pdf) and makes them available as:

- A Rust crate
- A browser-based interactive dashboard
- A Python package

## What's here now

The simple recuperated cycle design-point model is working, with three delivery layers:

- **Rust crate** — generic solver with CoolProp/RefProp support via [twine-models](https://github.com/isentropic-dev/twine-models), plus a plain-data facade for FFI consumers
- **Web dashboard** — interactive, runs entirely in your browser via WASM
- **Python package** — pip installable via PyO3 with CoolProp support (PR open, PyPI publishing coming soon)

## What's coming

This is active work — expect daily changes over the next few days.

- **Real-gas thermodynamic models** — CoolProp/RefProp integration via Twine, and a Rust port of FIT, which is a table-based interpolation over Helmholtz free energy for fast, accurate, and smooth fluid property calculations
- **Recompression cycle** design-point model
- **Off-design models** for both simple and recompression configurations
- **Parametric studies** in the dashboard

## Development

```bash
# Run tests
cargo test

# Build WASM and serve the dashboard locally (requires emscripten toolchain)
cargo build --target wasm32-unknown-emscripten --features wasm --release
cp target/wasm32-unknown-emscripten/release/brayton.wasm web/
python3 -m http.server 8080
# Open http://localhost:8080/web/
```

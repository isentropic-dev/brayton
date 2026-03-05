# Brayton Cycle Models

Brayton cycle models built on [Twine](https://github.com/isentropic-dev/twine-models).
While the models work with any fluid supported by [CoolProp](https://github.com/CoolProp/CoolProp), the primary use case is supercritical CO₂ power cycles, based on [this thesis](https://github.com/isentropic-dev/brayton/blob/main/docs/dyreby_thesis.pdf).

**[Try the interactive dashboard →](https://isentropic-dev.github.io/brayton/)**

## What's here

Design-point solvers for the simple recuperated and recompression Brayton cycles, available as:

- **Rust crate** — generic cycle solver with a plain-data facade for FFI consumers
- **Web dashboard** — interactive, runs entirely in your browser via WASM

## What's planned

- **Python package** — pip-installable via PyO3
- **Off-design models** for both cycle configurations

## Development

### WASM dashboard

The WASM build compiles CoolProp from source and requires the [Emscripten toolchain](https://emscripten.org/docs/getting_started/downloads.html).
Clone the CoolProp source into `vendor/` (one-time setup, gitignored):

```bash
git clone https://github.com/CoolProp/CoolProp.git vendor/CoolProp
```

Then build and serve:

```bash
COOLPROP_SOURCE_DIR=vendor/CoolProp cargo build --target wasm32-unknown-emscripten --features wasm --release
cp target/wasm32-unknown-emscripten/release/brayton.wasm web/
python3 -m http.server 8080
```

Open <http://localhost:8080/web/>.

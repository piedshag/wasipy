# WasiPy - Python Interpreter for WebAssembly

WasiPy is a Python interpreter that runs in WebAssembly using RustPython, enabling Python code execution in sandboxed environments.

## Overview

This project consists of two main components:
- **executor**: A WebAssembly module that contains the Python interpreter (RustPython)
- **server**: A Rust server that loads and executes the WebAssembly module to run Python code

## Building

### 1. Build the WebAssembly Executor

First, build the Python interpreter as a WebAssembly module:

```bash
cargo build --target wasm32-wasip2 -p executor --release
```

This creates `target/wasm32-wasip2/release/executor.wasm`.

### 2. Run the Server

The server will load the WebAssembly module and execute Python code. This will mount the current directory as read-only.

```bash
cargo run --release -p server --  -m .:.:ro -c "import os; print('Current directory contents:'); [print(f'  {item}') for item in os.listdir('.')]"
```

Now try it again without the `-m` flag.
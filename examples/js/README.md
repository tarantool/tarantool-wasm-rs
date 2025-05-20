# JavaScript Samples

> **Prerequisite:** You need `tarawasm` - either built locally from https://github.com/mandesero/tarawasm or used via the official Docker image mandeser0/tarawasm:latest.

1. **Copy the WASM file**  
    Copy `docs:adder@0.1.0.wasm` into your sample directory:  
    ```bash
    cp <path-to-tarantool-wasm-rs>/docs:adder@0.1.0.wasm <your-sample-dir>


1. **Initialize the project**
    Extract WIT definitions and save project config:

    ```bash
    cd <your-sample-dir>
    tarawasm init --lang js --wasm-file docs:adder@0.1.0.wasm adder
    ```

2. **Generate Go bindings**

    ```bash
    tarawasm bind
    ```

3. **Write your code**
    Edit `main.js` to implement your logic.

4. **Build the WASM module**

    ```bash
    tarawasm build
    ```

   This produces `adder.wasm` in the current directory.

5. **Run**

    ```bash
    tarantool <path-to-tarantool-wasm-rs>/run.lua <path-to-adder.wasm>
    ```

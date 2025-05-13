### To Create a New WASM Rust Component

1. **Refer to the official guide**:
   For more details on creating a WASM component in Rust, refer to the official Component Model documentation:
   [WASM Rust Component Guide](https://component-model.bytecodealliance.org/language-support/rust.html)

2. **Generate WIT from the `wit` directory**:

   * First, create a new component:

     ```bash
     $ cargo component new <name> --lib && cd <name>
     ```

   * Remove the default `world.wit` file:

     ```bash
     $ rm wit/world.wit
     ```

   * Build the WIT definitions using the specified WIT directory:

     ```bash
     $ wkg wit build --wit-dir <path-to-wit-dir>
     ```

   * Generate the `world.wit` file from the `adder` example:

     ```bash
     $ wasm-tools component wit docs:adder@0.1.0.wasm > wit/world.wit
     ```

   * Update `src/bindings.rs`:
     After generating the WIT definitions, update the bindings:

     ```bash
     $ cargo component bindings
     ```

3. **Write your code**:

   * Edit the `src/lib.rs` file to implement the logic for your component.

4. **Build the Component**:

   * Once the code is written, build the component with the following command:

     ```bash
     $ cargo component build --release
     ```
   * The compiled WASM file will be located in:

     ```bash
     target/wasm32-wasip1/release/<name>.wasm
     ```

5. **Run the Component**:

   * To run the component, use the following command from the root of your project:

     ```bash
     $ tarantool run.lua <path-to-target/wasm32-wasip1/release/<name>.wasm>
     ```

   * **Update `src/bindings.rs`**:
     After generating the WIT definitions, update the bindings:

     ```bash
     $ cargo component bindings
     ```
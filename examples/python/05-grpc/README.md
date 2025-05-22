```bash
pip install "grpclib[protobuf]"
```

1. **Wasm server**

   * Run the server in Tarantool:

     ```bash
     tarantool run.lua <path-to-server/adder.wasm>
     ```

     **Console output:**

     ```
     2025-05-22 15:45:13.460 [163176] main/104/run.lua/run run.lua:22 I> Load WASM module...
     2025-05-22 15:45:43.712 [163176] main/104/run.lua/run run.lua:24 I> Run WASM module...
     Serving on 0.0.0.0:50051
     ```
   * Run the client using system Python:

     ```bash
     python3 <path-to-server/client.py>
     ```

     **Console output:**

     ```
     Received: Hello, Alice!
     ```

2. **Wasm client**

   * Run the server using system Python:

     ```bash
     python3 <path-to-client/server.py>
     ```

     **Console output:**

     ```
     Serving on 127.0.0.1:50051
     ```
   * Run the client in Tarantool:

     ```bash
     tarantool run.lua <path-to-client/client.wasm>
     ```

     **Console output:**

     ```
     2025-05-22 15:52:28.116 [166765] main/104/run.lua/run run.lua:22 I> Load WASM module...
     2025-05-22 15:52:58.258 [166765] main/104/run.lua/run run.lua:24 I> Run WASM module...
     Received: Hello, Alice!
     2025-05-22 15:52:58.319 [166765] main/104/run.lua/run run.lua:27 I> WASM module finished...
     ```

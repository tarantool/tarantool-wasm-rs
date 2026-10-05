"""Strip, patch socket errors and validate repository components."""
from importlib import metadata
import json
from pathlib import Path
import subprocess
import sys

import twr_python_sdk

output = Path(sys.argv[1])
sdk = Path(twr_python_sdk.__file__).resolve().parent
data = json.loads((sdk / "sdk-manifest.json").read_text())
if metadata.version("componentize-py") != data["toolchain"]["componentize_py"]:
    raise ValueError("repository fixtures require the pinned componentize-py version")
if subprocess.check_output(["wasm-tools", "--version"], text=True).strip() != "wasm-tools " + data["toolchain"]["wasm_tools"]:
    raise ValueError("repository fixtures require the pinned wasm-tools version")
stripped = output.with_suffix(".strip.wasm")
try:
    subprocess.run(["wasm-tools", "strip", "--all", str(output), "-o", str(stripped)], check=True)
    twr_python_sdk.patch_socket_errors(stripped)
    subprocess.run(["wasm-tools", "validate", str(stripped)], check=True)
    stripped.replace(output)
finally:
    stripped.unlink(missing_ok=True)

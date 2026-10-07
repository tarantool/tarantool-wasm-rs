"""Preserve dependency feature annotations for componentize-py text parsing."""
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

source, output = map(Path, sys.argv[1:3])
world = sys.argv[3]
with tempfile.TemporaryDirectory() as resolved:
    subprocess.run([
        "wasm-tools", "component", "wit", str(source),
        "--all-features", "--out-dir", resolved,
    ], check=True)

    # Binary dependency merging can take an embedded interface definition
    # without its function stability annotations. Restore each direct package's
    # own text before componentize-py filters the requested @unstable features.
    packages = {path.read_text().splitlines()[0]: path
                for path in (Path(resolved) / "deps").glob("*.wit")}
    for dependency in sorted((source / "deps").glob("*.wasm")):
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run([
                "wasm-tools", "component", "wit", str(dependency),
                "--out-dir", directory,
            ], check=True)
            package, = Path(directory).glob("*.wit")
            declaration = package.read_text().splitlines()[0]
            shutil.copyfile(package, packages[declaration])

    # TLS still declares io@0.2.6. Merge compatible patch versions before
    # generating bindings so resources and Python module names stay unified.
    subprocess.run([
        "wasm-tools", "component", "wit", resolved, "--all-features",
        "--merge-world-imports-based-on-semver", world,
        "--out-dir", str(output),
    ], check=True)

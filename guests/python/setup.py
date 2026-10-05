"""Bundle compatibility ports from their authoritative source directories."""
from pathlib import Path
import shutil
from setuptools import setup
from setuptools.command.build_py import build_py

ROOT = Path(__file__).parent


class BuildPy(build_py):
    def run(self):
        # Reused setuptools output must not retain packages from older SDK names.
        if Path(self.build_lib).exists():
            shutil.rmtree(self.build_lib)
        super().run()
        package = Path(self.build_lib) / "twr_python_sdk"
        guest = package / "_guest"
        guest.mkdir(parents=True, exist_ok=True)
        for source in sorted((ROOT / "ports").glob("*/python/*.py")):
            shutil.copy2(source, guest / source.name)
        contract = package / "wit"
        contract.mkdir(exist_ok=True)
        shutil.copy2(ROOT / "ports/ssl/wit/ssl-bio.wit", contract / "ssl-bio.wit")
        shutil.copy2(ROOT / "sdk-manifest.json", package / "sdk-manifest.json")


setup(cmdclass={"build_py": BuildPy})

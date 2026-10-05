"""Native checks for snapshot imports whose build-time sources disappear."""
import importlib
import importlib.resources
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sdk/src"))
from twr_python_sdk.runtime import freeze_packages as helper


class FrozenPackageTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.meta_path = sys.meta_path[:]
        self.modules = helper._finder.modules.copy()
        self.resources = helper._finder.resources.copy()
        sys.path.insert(0, str(self.root))

    def tearDown(self):
        sys.path.remove(str(self.root))
        sys.meta_path[:] = self.meta_path
        helper._finder.modules.clear()
        helper._finder.modules.update(self.modules)
        helper._finder.resources.clear()
        helper._finder.resources.update(self.resources)
        for name in list(sys.modules):
            if name.startswith("_freeze_test_"):
                del sys.modules[name]
        self.directory.cleanup()
        importlib.invalidate_caches()

    def test_relative_imports_without_original_files(self):
        package = self.root / "_freeze_test_package"
        package.mkdir()
        (package / "__init__.py").write_text("from .child import value\n")
        (package / "child.py").write_text("value = 42\n")
        helper.freeze_packages("_freeze_test_package")
        self.assertNotIn("_freeze_test_package", sys.modules)
        shutil.rmtree(package)
        module = importlib.import_module("_freeze_test_package")
        self.assertEqual(module.value, 42)
        self.assertTrue(module.__file__.startswith("/frozen-packages/"))
        self.assertEqual(module.__loader__.get_source("_freeze_test_package.child"), "value = 42\n")
        self.assertEqual(importlib.reload(module).value, 42)

    def test_import_side_effect_runs_only_on_import(self):
        source = self.root / "_freeze_test_lazy.py"
        source.write_text("raise RuntimeError('runtime import reached')\n")
        helper.freeze_packages("_freeze_test_lazy")
        source.unlink()
        with self.assertRaisesRegex(RuntimeError, "runtime import reached"):
            importlib.import_module("_freeze_test_lazy")
        self.assertNotIn("_freeze_test_lazy", sys.modules)

    def test_bad_source_does_not_install_partial_snapshot(self):
        (self.root / "_freeze_test_first.py").write_text("value = 1\n")
        (self.root / "_freeze_test_bad.py").write_text("def broken(\n")
        with self.assertRaises(SyntaxError):
            helper.freeze_packages("_freeze_test_first", "_freeze_test_bad")
        self.assertNotIn("_freeze_test_first", helper._finder.modules)
        self.assertNotIn(helper._finder, sys.meta_path)
        self.assertFalse(any("_freeze_test_first" in path for path in helper._finder.resources))

    def test_resources_survive_source_removal_and_extract_with_cleanup(self):
        package = self.root / "_freeze_test_resources"
        package.mkdir()
        (package / "__init__.py").write_text("")
        (package / "nested").mkdir()
        (package / "nested" / "payload.bin").write_bytes(b"resource\0bytes")
        helper.freeze_packages("_freeze_test_resources")
        shutil.rmtree(package)
        module = importlib.import_module("_freeze_test_resources")
        root = importlib.resources.files(module)
        self.assertEqual([path.name for path in root.iterdir()], ["__init__.py", "nested"])
        resource = root.joinpath("nested", "payload.bin")
        self.assertEqual(resource.read_bytes(), b"resource\0bytes")
        with importlib.resources.as_file(resource) as extracted:
            self.assertEqual(extracted.read_bytes(), b"resource\0bytes")
        self.assertFalse(extracted.exists())
        with self.assertRaises(ValueError):
            root.joinpath("../private")
        with self.assertRaises(ValueError):
            resource.open("wb")

    def test_resource_limit_does_not_publish_partial_snapshot(self):
        package = self.root / "_freeze_test_limit"
        package.mkdir()
        (package / "__init__.py").write_text("value = 1\n")
        (package / "large.bin").write_bytes(b"x" * 64)
        with patch.object(helper, "_LIMIT", 32):
            with self.assertRaisesRegex(ValueError, "source/resource limit"):
                helper.freeze_packages("_freeze_test_limit")
        self.assertNotIn("_freeze_test_limit", helper._finder.modules)
        self.assertFalse(any("_freeze_test_limit" in path for path in helper._finder.resources))
        self.assertNotIn(helper._finder, sys.meta_path)


if __name__ == "__main__":
    unittest.main()

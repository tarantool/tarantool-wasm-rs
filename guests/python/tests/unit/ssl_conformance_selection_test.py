"""Upstream methods and decorators stay intact; tampered sources fail closed."""
import ast
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1] / 'conformance'
spec = importlib.util.spec_from_file_location('ssl_conformance_generator', ROOT / 'generate.py')
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)


class SelectionTests(unittest.TestCase):
    def test_selected_tests_and_helpers_have_identical_upstream_asts(self):
        selection = json.loads((ROOT / 'selection.json').read_text())
        original = ast.parse((ROOT / 'upstream/cpython-3.12.11/test_ssl.py').read_text())
        generated = ast.parse(generator.selected_module())
        upstream = {f'{cls.name}.{method.name}': method for cls in original.body if isinstance(cls, ast.ClassDef)
                    for method in cls.body if isinstance(method, ast.FunctionDef)}
        selected = {f'{cls}.{method}' for key in ('classes', 'helpers') for cls, methods in selection[key].items()
                    for method in methods}
        copies = {f'{cls.name}.{method.name}': method for cls in generated.body if isinstance(cls, ast.ClassDef)
                  and cls.name in selection['classes'] for method in cls.body if isinstance(method, ast.FunctionDef)}
        self.assertEqual(set(copies), selected)
        for name in selected:
            self.assertEqual(ast.dump(copies[name]), ast.dump(upstream[name]), name)
        for name in selection.get('module_helpers', []):
            original_node = next(node for node in original.body if isinstance(node, ast.FunctionDef) and node.name == name)
            generated_node = next(node for node in generated.body if isinstance(node, ast.FunctionDef) and node.name == name)
            self.assertEqual(ast.dump(original_node), ast.dump(generated_node), name)
        for name in selection.get('globals', []):
            def assignment(tree):
                return next(node for node in tree.body if isinstance(node, ast.Assign)
                            and any(isinstance(target, ast.Name) and target.id == name for target in node.targets))
            self.assertEqual(ast.dump(assignment(original)), ast.dump(assignment(generated)), name)

    def test_every_upstream_method_is_accounted_for(self):
        selection = json.loads((ROOT / 'selection.json').read_text())
        tree = ast.parse((ROOT / 'upstream/cpython-3.12.11/test_ssl.py').read_text())
        upstream = {f'{cls.name}.{method.name}' for cls in tree.body if isinstance(cls, ast.ClassDef)
                    for method in cls.body if isinstance(method, ast.FunctionDef) and method.name.startswith('test_')}
        selected = {f'{cls}.{method}' for cls, methods in selection['classes'].items() for method in methods}
        self.assertEqual(selected | set(selection['excluded']), upstream)
        self.assertFalse(selected & set(selection['excluded']))
        self.assertEqual(selection['upstream_test_count'], len(upstream))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'input'
            shutil.copytree(ROOT, root)
            selection['excluded'].pop(next(iter(selection['excluded'])))
            (root / 'selection.json').write_text(json.dumps(selection))
            with self.assertRaisesRegex(ValueError, 'unclassified upstream tests'):
                generator.selected_module(root)

    def test_support_tampering_and_missing_lifecycle_hooks_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'input'
            shutil.copytree(ROOT, root)
            selection = json.loads((root / 'selection.json').read_text())
            fixture = root / 'upstream/cpython-3.12.11' / next(iter(selection['support_files']))
            original = fixture.read_bytes()
            fixture.write_bytes(b'replaced support')
            with self.assertRaisesRegex(ValueError, 'support checksum mismatch'):
                generator.selected_module(root)
            fixture.write_bytes(original)
            selection['classes']['SimpleBackgroundTests'] = ['test_connect']
            (root / 'selection.json').write_text(json.dumps(selection))
            with self.assertRaisesRegex(ValueError, 'unselected lifecycle hook'):
                generator.selected_module(root)

    def test_fixture_tampering_and_invalid_paths_fail_explicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'input'
            shutil.copytree(ROOT, root)
            selection = json.loads((root / 'selection.json').read_text())
            name = next(iter(selection['data_files']))
            fixture = root / 'upstream/cpython-3.12.11' / name
            fixture.write_bytes(b'replaced fixture')
            with self.assertRaisesRegex(ValueError, 'fixture checksum mismatch'):
                generator.selected_module(root)
            selection['data_files'] = {'../outside': 'unused'}
            (root / 'selection.json').write_text(json.dumps(selection))
            with self.assertRaisesRegex(ValueError, 'invalid fixture path'):
                generator.selected_module(root)

    def test_missing_module_helpers_and_globals_fail_explicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'input'
            shutil.copytree(ROOT, root)
            selection = json.loads((root / 'selection.json').read_text())
            selection['module_helpers'].append('missing_helper')
            (root / 'selection.json').write_text(json.dumps(selection))
            with self.assertRaisesRegex(ValueError, 'module helpers missing upstream'):
                generator.selected_module(root)
            selection['module_helpers'].remove('missing_helper')
            selection['globals'].append('MISSING_GLOBAL')
            (root / 'selection.json').write_text(json.dumps(selection))
            with self.assertRaisesRegex(ValueError, 'globals missing upstream'):
                generator.selected_module(root)

    def test_missing_selection_and_source_tampering_fail_explicitly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / 'input'
            shutil.copytree(ROOT, root)
            selection = json.loads((root / 'selection.json').read_text())
            selection['classes']['MemoryBIOTests'].append('test_missing')
            (root / 'selection.json').write_text(json.dumps(selection))
            with self.assertRaisesRegex(ValueError, 'missing upstream'):
                generator.selected_module(root)
            (root / 'upstream/cpython-3.12.11/test_ssl.py').write_text('# replaced')
            with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
                generator.selected_module(root)


if __name__ == '__main__':
    unittest.main()

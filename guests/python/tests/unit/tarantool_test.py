"""Failure-path checks; host behavior is exercised by the Wasm fixture."""

from dataclasses import dataclass
from pathlib import Path
from types import SimpleNamespace, ModuleType
from unittest.mock import Mock, patch
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "sdk/src"))
from twr_python_sdk import tarantool as box
from twr_python_sdk.tarantool import _codec
from twr_python_sdk.tarantool._bindings import Bindings
from twr_python_sdk.tarantool.database import row, IndexIterator
from twr_python_sdk.tarantool.tuples import Tuple


@dataclass(frozen=True)
class Err(Exception):
    value: object


class ApiTests(unittest.TestCase):
    def setUp(self):
        types = ModuleType("componentize_py_types")
        types.Err = Err
        self.stub = patch.dict(sys.modules, {"componentize_py_types": types})
        self.stub.start()
        self.addCleanup(self.stub.stop)

    def test_packaged_callback_contract_matches_host(self):
        repo = Path(__file__).resolve().parents[4]
        source = (repo / "wit/adder.wit").read_text()
        expected = (
            source.splitlines()[0]
            + "\n\ninterface lua-callback {"
            + source.split("interface lua-callback {", 1)[1]
        )
        packaged = repo / "guests/python/sdk/src/twr_python_sdk/wit/lua-callback.wit"
        self.assertEqual(packaged.read_text(), expected)

    def test_extension_key_is_a_scalar(self):
        from msgpack import ExtType

        extension = ExtType(127, b"opaque")
        self.assertEqual(_codec.unpack(_codec.key(extension)), [extension])

    def test_error_preserves_host_metadata_and_cause(self):
        record = SimpleNamespace(
            code=78, message="failure", error_type="ClientError", file="x.lua", line=13
        )
        source = Err(record)
        with self.assertRaises(box.TarantoolError) as caught:
            Bindings.invoke(Mock(side_effect=source))
        result = caught.exception
        self.assertEqual(
            (result.code, result.message, result.error_type, result.file, result.line),
            (78, "failure", "ClientError", "x.lua", 13),
        )
        self.assertIs(result.__cause__, source)

    def test_callback_error_preserves_cause(self):
        source = Err("callback failed")
        with self.assertRaisesRegex(box.LuaCallbackError, "callback failed") as caught:
            Bindings.invoke(Mock(side_effect=source))
        self.assertIs(caught.exception.__cause__, source)

    def test_row_released_on_decode_failure(self):
        release = Mock()
        bindings = Bindings(
            {"box_tuple": SimpleNamespace(to_bytes=lambda _: b"\xc1", release=release)}
        )
        with self.assertRaises(ValueError):
            row(bindings, 123)
        release.assert_called_once_with(123)

    def test_row_released_on_host_serialization_failure(self):
        release = Mock()
        record = SimpleNamespace(
            code=1, message="bad tuple", error_type="ClientError", file=None, line=None
        )
        bindings = Bindings(
            {
                "box_tuple": SimpleNamespace(
                    to_bytes=Mock(side_effect=Err(record)), release=release
                )
            }
        )
        with self.assertRaises(box.TarantoolError):
            row(bindings, 123)
        release.assert_called_once_with(123)

    def test_index_iterator_closes_on_decode_failure(self):
        handle = SimpleNamespace(next=lambda: 123, __exit__=Mock())
        release = Mock()
        bindings = Bindings(
            {"box_tuple": SimpleNamespace(to_bytes=lambda _: b"\xc1", release=release)}
        )
        iterator = IndexIterator(bindings, handle)
        with self.assertRaises(ValueError):
            next(iterator)
        self.assertTrue(iterator.closed)
        handle.__exit__.assert_called_once()
        release.assert_called_once()

    def test_tuple_copy_survives_original_close_and_close_is_idempotent(self):
        from copy import copy

        refs = [1]

        def retain(_):
            refs[0] += 1

        def release(_):
            refs[0] -= 1

        bindings = Bindings(
            {
                "box_tuple": SimpleNamespace(
                    retain=retain, release=release, to_bytes=lambda _: _codec.pack([42])
                )
            }
        )
        original = Tuple(bindings, 1)
        retained = copy(original)
        original.close()
        original.close()
        self.assertEqual(retained.to_tuple(), (42,))
        with self.assertRaises(box.ClosedResourceError):
            original.copy()
        retained.close()
        self.assertEqual(refs[0], 0)

    def test_iterators_and_callbacks_cannot_be_copied_or_pickled(self):
        from copy import copy, deepcopy
        import pickle

        for cls in (IndexIterator, box.LuaCallback):
            handle = SimpleNamespace(__exit__=Mock())
            resource = cls(Bindings(), handle)
            for function in (copy, deepcopy, pickle.dumps):
                with self.assertRaises(TypeError):
                    function(resource)
            self.assertFalse(resource.closed)
            resource.close()
            handle.__exit__.assert_called_once()

    def test_owned_handle_copies_acquire_separate_ownership(self):
        from copy import copy, deepcopy
        import pickle

        for cls, interface, copier, copied_handle in (
            (box.KeyDef, "key_def", "duplicate", 2),
            (box.TupleFormat, "tuple_format", "retain", 1),
        ):
            release = Mock()
            module = SimpleNamespace(release=release)
            setattr(module, copier, Mock(return_value=copied_handle))
            original = cls(Bindings({interface: module}), 1)
            copied = copy(original)
            self.assertIsNot(copied, original)
            getattr(module, copier).assert_called_once_with(1)
            for function in (deepcopy, pickle.dumps):
                with self.assertRaises(TypeError):
                    function(original)
            original.close()
            self.assertFalse(copied.closed)
            copied.close()
            self.assertEqual(release.call_count, 2)

    def test_cleanup_does_not_replace_application_error(self):
        bindings = Bindings(
            {
                "box_tuple": SimpleNamespace(
                    release=Mock(side_effect=RuntimeError("cleanup"))
                )
            }
        )
        with self.assertRaisesRegex(ValueError, "application"):
            with Tuple(bindings, 1):
                raise ValueError("application")

    def test_cross_binding_resources_rejected(self):
        first, second = Bindings(), Bindings()
        left, right = Tuple(first, 1), Tuple(second, 2)
        definition = box.KeyDef(first, 3)
        with self.assertRaisesRegex(ValueError, "different"):
            left.compare(right, definition)
        # Avoid finalizer host calls on these deliberately invalid test handles.
        left._handle = right._handle = definition._handle = None

    def test_keys_and_binary_are_not_json_encoded(self):
        self.assertEqual(_codec.unpack(_codec.key(b"\x00\xff")), [b"\x00\xff"])
        self.assertEqual(_codec.unpack(_codec.key([1, "a"])), [1, "a"])
        self.assertEqual(
            _codec.unpack(_codec.pack({1: [None, True]})), {1: [None, True]}
        )

    def test_tuple_update_normalizes_zero_based_fields_without_mutating_input(self):
        operations = [("=", 0, 42), ("+", -1, 1)]
        self.assertEqual(
            _codec.unpack(_codec.operations(operations)), [["=", 0, 42], ["+", -1, 1]]
        )
        self.assertEqual(
            _codec.unpack(_codec.operations(operations, tuple_api=True)),
            [["=", 1, 42], ["+", -1, 1]],
        )
        self.assertEqual(operations, [("=", 0, 42), ("+", -1, 1)])

    def test_named_space_and_index_resolve_again_after_schema_change(self):
        space_id = [10]
        database = SimpleNamespace(
            space_by_name=lambda name: SimpleNamespace(id=space_id[0]),
            index_by_name=lambda space, name: SimpleNamespace(id=0, space_id=space.id),
        )
        client = box.Tarantool(bindings={"database": database})
        space = client.space["users"]
        index = space.index["primary"]
        self.assertEqual(index._resolve().space_id, 10)
        space_id[0] = 20
        self.assertEqual(space.id, 20)
        self.assertEqual(index._resolve().space_id, 20)

    def transactions(
        self, *, active=None, begin_error=None, commit_error=None, rollback_error=None
    ):
        state = [active]

        def begin():
            if begin_error:
                raise begin_error
            state[0] = 123

        def commit():
            state[0] = None
            if commit_error:
                raise commit_error

        def rollback():
            state[0] = None
            if rollback_error:
                raise rollback_error

        host = SimpleNamespace(
            id=lambda: state[0],
            begin=Mock(side_effect=begin),
            commit=Mock(side_effect=commit),
            rollback=Mock(side_effect=rollback),
        )
        return box.Tarantool(bindings={"transaction": host}).transaction, host, state

    def test_nested_transaction_does_not_end_owner(self):
        tx, host, state = self.transactions()
        with tx():
            with self.assertRaises(RuntimeError):
                with tx():
                    pass
            self.assertEqual(state[0], 123)
            host.rollback.assert_not_called()
        host.commit.assert_called_once()

    def test_failed_begin_does_not_rollback(self):
        tx, host, _ = self.transactions(begin_error=ValueError("begin"))
        with self.assertRaisesRegex(ValueError, "begin"):
            with tx():
                pass
        host.rollback.assert_not_called()

    def test_failed_commit_does_not_attempt_to_rollback_an_ended_transaction(self):
        tx, host, state = self.transactions(commit_error=ValueError("commit"))
        with self.assertRaisesRegex(ValueError, "commit"):
            with tx():
                pass
        host.rollback.assert_not_called()
        self.assertIsNone(state[0])

    def test_failed_rollback_preserves_application_exception(self):
        tx, host, _ = self.transactions(rollback_error=ValueError("rollback"))
        with self.assertRaisesRegex(KeyError, "application"):
            with tx():
                raise KeyError("application")
        host.rollback.assert_called_once()

    def test_replaced_transaction_is_not_committed_or_rolled_back(self):
        tx, host, state = self.transactions()
        with self.assertRaisesRegex(RuntimeError, "replaced"):
            with tx():
                state[0] = 456
        host.commit.assert_not_called()
        host.rollback.assert_not_called()

    def test_raw_bindings_remain_raw(self):
        import _tarantool

        original = dict(_tarantool._modules)
        self.addCleanup(
            lambda: (_tarantool._modules.clear(), _tarantool._modules.update(original))
        )
        raw = SimpleNamespace(get=Mock(side_effect=Err("raw error")))
        _tarantool.configure(index=raw)
        self.assertIs(_tarantool.index, raw)
        with self.assertRaises(Err):
            _tarantool.index.get()
        with self.assertRaises(ValueError):
            _tarantool.configure(unknown=raw)


if __name__ == "__main__":
    unittest.main()

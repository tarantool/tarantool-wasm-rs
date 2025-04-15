
from adder import exports
from adder.imports import say, key_def, msgpack, tuple_format
from adder.imports.tarantool_tarantool_types import KeyPart, KeyPartFlags

def test_tuple_format():
    say.say_info("PY | Test TupleFormat")

    # Создание KeyPart-ов
    parts = [
        KeyPart(
            field_no=0,
            field_type="unsigned",
            collation=None,
            path=None,
            flags=KeyPartFlags.IS_NULLABLE
        ),
        KeyPart(
            field_no=1,
            field_type="string",
            collation=None,
            path=None,
            flags=KeyPartFlags.EXCLUDE_NULL
        ),
    ]

    kd = key_def.new(parts)
    say.say_info(f"PY | Created key_def: {kd}")

    tf = tuple_format.new([kd])
    say.say_info(f"PY | Created tuple_format: {tf}")

    tuple_format.ref(tf)
    say.say_info("PY | Called tuple_format.ref")

    tuple_format.unref(tf)
    say.say_info("PY | Called tuple_format.unref")

    default_tf = tuple_format.default()
    say.say_info(f"PY | Default tuple_format: {default_tf}")
    
class Run(exports.Run):
    def run(self) -> None:
        try:
            test_tuple_format()
        except Exception as e:
            say.say_error(str(e))

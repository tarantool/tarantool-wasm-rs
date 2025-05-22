
from wit_world import exports
from wit_world.imports import say, key_def, tuple_format
from wit_world.imports.tarantool_tarantool_types import KeyPart, KeyPartFlags


class IncomingHandler(exports.IncomingHandler):
    def handle(self, request, response_out):
        pass


def test_tuple_format():
    say.say_info("PY | Test TupleFormat", None)

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
    say.say_info(f"PY | Created key_def: {kd}", None)

    tf = tuple_format.new([kd])
    say.say_info(f"PY | Created tuple_format: {tf}", None)

    tuple_format.ref(tf)
    say.say_info("PY | Called tuple_format.ref", None)

    tuple_format.unref(tf)
    say.say_info("PY | Called tuple_format.unref", None)

    default_tf = tuple_format.default()
    say.say_info(f"PY | Default tuple_format: {default_tf}", None)
    
class Run(exports.Run):
    def run(self) -> None:
        try:
            test_tuple_format()
        except Exception as e:
            say.say_error(str(e), None)

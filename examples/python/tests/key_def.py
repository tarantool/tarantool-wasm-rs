from adder import exports
from adder.imports import say, docs_adder_error as error, key_def, msgpack
from adder.imports.docs_adder_types import BoxError, KeyPart, KeyPartFlags
import json

def encode(obj):
    try:
        return msgpack.encode(json.dumps(obj).encode('utf-8'))
    except Exception as error:
        say.say_error(f"PY | Error in encode: {json.dumps({'error': str(error)}, indent=2)}")
        raise


def decode(*, data = None, ptr = None):
    try:
        if ptr:
            decoded_json_bytes: bytes = msgpack.decode_from_raw_ptr(ptr)
        elif data:
            decoded_json_bytes: bytes = msgpack.decode(data)
        decoded_json_string = decoded_json_bytes.decode('utf-8')
        return json.loads(decoded_json_string)
    except Exception as error:
        say.say_error(f"PY | Error in decode: {json.dumps({'error': str(error)}, indent=2)}")
        raise

def test_key_def():
    say.say_info("PY | Test KeyDef")

    key_parts_1 = [
        KeyPart(
            field_no=0,
            field_type="unsigned",
            collation=None,
            path=None,
            flags=KeyPartFlags.IS_NULLABLE
        ),
        KeyPart(
            field_no=3,
            field_type="string",
            collation="unicode",
            path="t1",
            flags=KeyPartFlags.EXCLUDE_NULL
        ),
    ]

    key_parts_2 = [
        KeyPart(
            field_no=1,
            field_type="string",
            collation=None,
            path=None,
            flags=KeyPartFlags.EXCLUDE_NULL
        )
    ]

    t1 = key_def.new(key_parts_1)
    parts_dump_1 = key_def.dump_parts(t1)
    parts_str_1 = "\n".join("\t" + str(p) for p in parts_dump_1)
    say.say_info(f"PY | Created key_def: t1 = {t1}")
    say.say_info(f"PY | Key parts count: t1 = {key_def.part_count(t1)}")
    say.say_info(f"PY | t1 dump_parts:\n{parts_str_1}")

    t2 = key_def.new(key_parts_2)
    say.say_info(f"PY | Created key_def: t2 = {t2}")
    parts_dump_2 = key_def.dump_parts(t2)
    parts_str_2 = "\n".join("\t" + str(p) for p in parts_dump_2)
    say.say_info(f"PY | Key parts count: t2 = {key_def.part_count(t2)}")
    say.say_info(f"PY | t2 dump_parts:\n{parts_str_2}")

    t1_dup = key_def.dup(t1)
    say.say_info(f"PY | t1_dup duplicated: {t1_dup}")
    parts_dump_dup = key_def.dump_parts(t1_dup)
    parts_str_dup = "\n".join("\t" + str(p) for p in parts_dump_dup)
    say.say_info(f"PY | Created key_def: t1_dup = {t1_dup}")
    say.say_info(f"PY | Key parts count: t1_dup = {key_def.part_count(t1_dup)}")
    say.say_info(f"PY | t1_dup dump_parts:\n{parts_str_dup}")

    merged_t = key_def.merge(t1, t2)
    say.say_info(f"PY | Merged t1 and t2 → merged_t = {merged_t}")
    parts_dump_merged = key_def.dump_parts(merged_t)
    parts_str_merged = "\n".join("\t" + str(p) for p in parts_dump_merged)
    say.say_info(f"PY | Created key_def: merged_t = {merged_t}")
    say.say_info(f"PY | Key parts count: merged_t = {key_def.part_count(merged_t)}")
    say.say_info(f"PY | merged_t dump_parts:\n{parts_str_merged}")

    valid_key = encode([1, "abc"])
    invalid_key = encode(["xxx"])

    is_valid, size = key_def.validate_key(t1, valid_key)
    say.say_info(f"PY | validate_key(valid_key): {is_valid}, size = {size}")

    is_valid_full, size_full = key_def.validate_full_key(t1, valid_key)
    say.say_info(f"PY | validate_full_key(valid_key): {is_valid_full}, size = {size_full}")

    try:
        key_def.validate_key(t1, invalid_key)
    except Exception as e:
        say.say_error(f"PY | validate_key(invalid_key) error: {str(e)}")

    key_def.delete(t1)
    key_def.delete(t2)
    key_def.delete(t1_dup)
    key_def.delete(merged_t)
    say.say_info("PY | Deleted all key_defs")


class Run(exports.Run):
    def run(self) -> None:
        try:
            test_key_def()
        except Exception as e:
            say.say_error(str(e))

#include "adder.h"
#include <string.h>

bool exports_adder_t_tuple_new(adder_list_u8_t *data, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_new(data, ret, err);
}

bool exports_adder_t_new(adder_list_key_part_t *parts, adder_key_def_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_new((tarantool_tarantool_key_def_list_key_part_t *)parts, ret, err);
}

bool exports_adder_t_dup(adder_key_def_t *key_def, adder_key_def_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_dup(key_def, ret, err);
}

bool exports_adder_t_merge(adder_key_def_t *left, adder_key_def_t *right, adder_key_def_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_merge(left, right, ret, err);
}

void exports_adder_t_delete(adder_key_def_t *key_def) {
    tarantool_tarantool_key_def_delete(key_def);
}

bool exports_adder_t_dump_parts(adder_key_def_t *key_def, adder_list_key_part_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_dump_parts(key_def, (tarantool_tarantool_key_def_list_key_part_t *)ret, err);
}

bool exports_adder_t_validate_key(adder_key_def_t *key_def, adder_list_u8_t *key, adder_tuple2_bool_u32_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_validate_key(key_def, key, ret, err);
}

bool exports_adder_t_validate_full_key(adder_key_def_t *key_def, adder_list_u8_t *key, adder_tuple2_bool_u32_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_validate_full_key(key_def, key, ret, err);
}

bool exports_adder_t_part_count(adder_key_def_t *key_def, uint32_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_part_count(key_def, ret, err);
}

bool exports_adder_t_extract_key(adder_key_def_t *key_def, adder_box_tuple_t *tuple, adder_list_u8_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_extract_key(key_def, tuple, ret, err);
}

bool exports_adder_t_validate_tuple(adder_key_def_t *key_def, adder_box_tuple_t *tuple) {
    return tarantool_tarantool_key_def_validate_tuple(key_def, tuple);
}

void exports_adder_decode_from_raw_ptr(uint64_t ptr, adder_list_u8_t *ret) {
    tarantool_tarantool_msgpack_decode_from_raw_ptr(ptr, ret, NULL);
}

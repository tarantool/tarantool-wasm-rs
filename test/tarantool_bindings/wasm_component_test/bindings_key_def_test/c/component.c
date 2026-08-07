#include "bindings/adder.h"

bool exports_adder_t_tuple_new(adder_msgpack_value_t *data,
                               adder_box_tuple_t *ret,
                               adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_new(data, ret, err);
}

bool exports_adder_t_new(adder_list_key_part_t *parts,
                         adder_key_def_t *ret,
                         adder_box_error_t *err) {
    return tarantool_tarantool_key_def_new(
        (tarantool_tarantool_key_def_list_key_part_t *)parts, ret, err);
}

bool exports_adder_t_duplicate(adder_key_def_t *key_def,
                               adder_key_def_t *ret,
                               adder_box_error_t *err) {
    return tarantool_tarantool_key_def_duplicate(key_def, ret, err);
}

bool exports_adder_t_merge(adder_key_def_t *left,
                           adder_key_def_t *right,
                           adder_key_def_t *ret,
                           adder_box_error_t *err) {
    return tarantool_tarantool_key_def_merge(left, right, ret, err);
}

void exports_adder_t_release(adder_key_def_t *key_def) {
    tarantool_tarantool_key_def_release(key_def);
}

bool exports_adder_t_dump_parts(adder_key_def_t *key_def,
                                adder_list_key_part_t *ret,
                                adder_box_error_t *err) {
    return tarantool_tarantool_key_def_dump_parts(
        key_def, (tarantool_tarantool_key_def_list_key_part_t *)ret, err);
}

bool exports_adder_t_validate_key(adder_key_def_t *key_def,
                                  adder_msgpack_value_t *key,
                                  adder_box_error_t *err) {
    return tarantool_tarantool_key_def_validate_key(key_def, key, err);
}

bool exports_adder_t_validate_full_key(adder_key_def_t *key_def,
                                       adder_msgpack_value_t *key,
                                       adder_box_error_t *err) {
    return tarantool_tarantool_key_def_validate_full_key(key_def, key, err);
}

bool exports_adder_t_part_count(adder_key_def_t *key_def,
                                uint32_t *ret,
                                adder_box_error_t *err) {
    return tarantool_tarantool_key_def_part_count(key_def, ret, err);
}

bool exports_adder_t_extract_key(adder_key_def_t *key_def,
                                 adder_box_tuple_t *tuple,
                                 adder_msgpack_value_t *ret,
                                 adder_box_error_t *err) {
    return tarantool_tarantool_key_def_extract_key(key_def, tuple, ret, err);
}

bool exports_adder_t_validate_tuple(adder_key_def_t *key_def,
                                    adder_box_tuple_t *tuple,
                                    adder_box_error_t *err) {
    return tarantool_tarantool_key_def_validate_tuple(key_def, tuple, err);
}

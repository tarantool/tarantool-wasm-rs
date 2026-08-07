#include "bindings/adder.h"
#include <string.h>

bool exports_adder_t_key_def_new(adder_list_key_part_t *parts, adder_key_def_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_new((tarantool_tarantool_key_def_list_key_part_t *)parts, ret, err);
}

void exports_adder_t_default(adder_tuple_format_t *ret) {
    tarantool_tarantool_tuple_format_default(ret);
}

bool exports_adder_t_new(adder_list_key_def_t *key_defs, adder_tuple_format_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_tuple_format_new(
        (tarantool_tarantool_tuple_format_list_key_def_t *) key_defs,
        ret,
        err);
}

void exports_adder_t_ref(adder_tuple_format_t *tf) {
    tarantool_tarantool_tuple_format_retain(tf);
}

void exports_adder_t_unref(adder_tuple_format_t *tf) {
    tarantool_tarantool_tuple_format_release(tf);
}

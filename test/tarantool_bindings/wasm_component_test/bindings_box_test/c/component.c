#include "adder.h"

int64_t exports_adder_t_schema_version(void) {
    return tarantool_tarantool_ttbox_schema_version();
}

bool exports_adder_t_space_by_name(adder_string_t *name, adder_space_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_ttbox_space_by_name(name, ret, err);
}

bool exports_adder_t_index_by_name(adder_space_t *space, adder_string_t *name, adder_index_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_ttbox_index_by_name((tarantool_tarantool_ttbox_space_t *) space, name, ret, err);
}

bool exports_adder_t_insert(adder_space_t *space, adder_list_u8_t *tup, adder_list_u8_t *ret, adder_box_error_t *err) {
    adder_box_tuple_t tuple;
    if (!tarantool_tarantool_ttbox_insert((tarantool_tarantool_ttbox_space_t *) space, tup, &tuple, err)) {
        return false;
    }
    bool ok = tarantool_tarantool_box_tuple_to_buf(&tuple, ret, err);
    return ok;
}

bool exports_adder_t_update(adder_index_t *index, adder_list_u8_t *key, adder_list_u8_t *ops, adder_list_u8_t *ret, adder_box_error_t *err) {
    adder_box_tuple_t tuple;
    if (!tarantool_tarantool_ttbox_update((tarantool_tarantool_ttbox_index_t *) index, key, ops, &tuple, err)) {
        return false;
    }
    bool ok = tarantool_tarantool_box_tuple_to_buf(&tuple, ret, err);
    return ok;
}

bool exports_adder_t_replace(adder_space_t *space, adder_list_u8_t *tup, adder_list_u8_t *ret, adder_box_error_t *err) {
    adder_box_tuple_t tuple;
    if (!tarantool_tarantool_ttbox_replace((tarantool_tarantool_ttbox_space_t *) space, tup, &tuple, err)) {
        return false;
    }
    bool ok = tarantool_tarantool_box_tuple_to_buf(&tuple, ret, err);
    return ok;
}

bool exports_adder_t_upsert(adder_index_t *index, adder_list_u8_t *tup, adder_list_u8_t *ops, adder_box_error_t *err) {
    return tarantool_tarantool_ttbox_upsert((tarantool_tarantool_ttbox_index_t *) index, tup, ops, err);
}

bool exports_adder_t_delete(adder_index_t *index, adder_list_u8_t *key, adder_list_u8_t *ret, adder_box_error_t *err) {
    adder_box_tuple_t tuple;
    if (!tarantool_tarantool_ttbox_delete((tarantool_tarantool_ttbox_index_t *) index, key, &tuple, err)) {
        return false;
    }
    bool ok = tarantool_tarantool_box_tuple_to_buf(&tuple, ret, err);
    return ok;
}

bool exports_adder_t_truncate(adder_space_t *space, adder_box_error_t *err) {
    return tarantool_tarantool_ttbox_truncate((tarantool_tarantool_ttbox_space_t *) space, err);
}

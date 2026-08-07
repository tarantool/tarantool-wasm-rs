#include "bindings/adder.h"

uint32_t exports_adder_t_schema_version(void) {
    return tarantool_tarantool_database_schema_version();
}

bool exports_adder_t_space_by_name(adder_string_t *name, adder_option_space_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_database_space_by_name(
        name, (tarantool_tarantool_database_option_space_t *) ret, err);
}

bool exports_adder_t_index_by_name(adder_space_t *space, adder_string_t *name, adder_option_index_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_database_index_by_name(
        (tarantool_tarantool_database_space_t *) space,
        name,
        (tarantool_tarantool_database_option_index_t *) ret,
        err);
}

bool exports_adder_t_insert(adder_space_t *space, adder_msgpack_value_t *tup, adder_msgpack_value_t *ret, adder_box_error_t *err) {
    tarantool_tarantool_box_tuple_box_tuple_t tuple;
    if (!tarantool_tarantool_database_insert((tarantool_tarantool_database_space_t *) space, tup, &tuple, err)) {
        return false;
    }
    bool ok = tarantool_tarantool_box_tuple_to_bytes(&tuple, ret, err);
    tarantool_tarantool_box_tuple_release(&tuple);
    return ok;
}

bool exports_adder_t_update(adder_index_t *index, adder_msgpack_value_t *key, adder_msgpack_value_t *ops, adder_option_msgpack_value_t *ret, adder_box_error_t *err) {
    tarantool_tarantool_database_option_box_tuple_t tuple;
    if (!tarantool_tarantool_database_update(
            (tarantool_tarantool_database_index_t *) index, key, ops, &tuple, err)) {
        return false;
    }
    ret->is_some = tuple.is_some;
    if (!tuple.is_some) {
        return true;
    }
    bool ok = tarantool_tarantool_box_tuple_to_bytes(&tuple.val, &ret->val, err);
    tarantool_tarantool_box_tuple_release(&tuple.val);
    return ok;
}

bool exports_adder_t_replace(adder_space_t *space, adder_msgpack_value_t *tup, adder_msgpack_value_t *ret, adder_box_error_t *err) {
    tarantool_tarantool_box_tuple_box_tuple_t tuple;
    if (!tarantool_tarantool_database_replace((tarantool_tarantool_database_space_t *) space, tup, &tuple, err)) {
        return false;
    }
    bool ok = tarantool_tarantool_box_tuple_to_bytes(&tuple, ret, err);
    tarantool_tarantool_box_tuple_release(&tuple);
    return ok;
}

bool exports_adder_t_upsert(adder_index_t *index, adder_msgpack_value_t *tup, adder_msgpack_value_t *ops, adder_box_error_t *err) {
    return tarantool_tarantool_database_upsert((tarantool_tarantool_database_index_t *) index, tup, ops, err);
}

bool exports_adder_t_delete(adder_index_t *index, adder_msgpack_value_t *key, adder_option_msgpack_value_t *ret, adder_box_error_t *err) {
    tarantool_tarantool_database_option_box_tuple_t tuple;
    if (!tarantool_tarantool_database_delete(
            (tarantool_tarantool_database_index_t *) index, key, &tuple, err)) {
        return false;
    }
    ret->is_some = tuple.is_some;
    if (!tuple.is_some) {
        return true;
    }
    bool ok = tarantool_tarantool_box_tuple_to_bytes(&tuple.val, &ret->val, err);
    tarantool_tarantool_box_tuple_release(&tuple.val);
    return ok;
}

bool exports_adder_t_truncate(adder_space_t *space, adder_box_error_t *err) {
    return tarantool_tarantool_database_truncate((tarantool_tarantool_database_space_t *) space, err);
}

#include "bindings/adder.h"

static bool tuple_to_msgpack(tarantool_tarantool_index_option_box_tuple_t *tuple,
                             adder_option_msgpack_value_t *ret,
                             adder_box_error_t *err) {
    if (!tuple->is_some) {
        ret->is_some = false;
        return true;
    }

    ret->is_some = true;
    bool ok = tarantool_tarantool_box_tuple_to_bytes(&tuple->val,
                                                      &ret->val,
                                                      err);
    tarantool_tarantool_box_tuple_release(&tuple->val);
    return ok;
}

bool exports_adder_t_len(adder_index_t *index,
                         uint64_t *ret,
                         adder_box_error_t *err) {
    return tarantool_tarantool_index_tuple_count(index, ret, err);
}

bool exports_adder_t_bsize(adder_index_t *index,
                           uint64_t *ret,
                           adder_box_error_t *err) {
    return tarantool_tarantool_index_memory_size(index, ret, err);
}

bool exports_adder_t_random(adder_index_t *index,
                            uint32_t rnd,
                            adder_option_msgpack_value_t *ret,
                            adder_box_error_t *err) {
    tarantool_tarantool_index_option_box_tuple_t tuple;
    if (!tarantool_tarantool_index_random(index, rnd, &tuple, err)) {
        return false;
    }
    return tuple_to_msgpack(&tuple, ret, err);
}

bool exports_adder_t_get(adder_index_t *index,
                         adder_msgpack_value_t *key,
                         adder_option_msgpack_value_t *ret,
                         adder_box_error_t *err) {
    tarantool_tarantool_index_option_box_tuple_t tuple;
    if (!tarantool_tarantool_index_get(index, key, &tuple, err)) {
        return false;
    }
    return tuple_to_msgpack(&tuple, ret, err);
}

bool exports_adder_t_min(adder_index_t *index,
                         adder_msgpack_value_t *key,
                         adder_option_msgpack_value_t *ret,
                         adder_box_error_t *err) {
    tarantool_tarantool_index_option_box_tuple_t tuple;
    if (!tarantool_tarantool_index_min(index, key, &tuple, err)) {
        return false;
    }
    return tuple_to_msgpack(&tuple, ret, err);
}

bool exports_adder_t_max(adder_index_t *index,
                         adder_msgpack_value_t *key,
                         adder_option_msgpack_value_t *ret,
                         adder_box_error_t *err) {
    tarantool_tarantool_index_option_box_tuple_t tuple;
    if (!tarantool_tarantool_index_max(index, key, &tuple, err)) {
        return false;
    }
    return tuple_to_msgpack(&tuple, ret, err);
}

bool exports_adder_t_count(adder_index_t *index,
                           adder_iterator_type_t iter_type,
                           adder_msgpack_value_t *key,
                           uint64_t *ret,
                           adder_box_error_t *err) {
    return tarantool_tarantool_index_count(index, iter_type, key, ret, err);
}

#include "adder.h"

bool exports_adder_t_len(adder_index_t *index, int64_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_index_len(index, ret, err);
}

bool exports_adder_t_bsize(adder_index_t *index, int64_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_index_bsize(index, ret, err);
}

bool exports_adder_t_count(adder_index_t *index, adder_iterator_type_t iter_type, adder_list_u8_t *key, int64_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_index_count(index, iter_type, key, ret, err);
}

static bool index_to_buf(
    bool (*getter)(adder_index_t *, void *, adder_box_tuple_t *, adder_box_error_t *),
    adder_index_t *index, void *key_or_rnd,
    adder_list_u8_t *ret, adder_box_error_t *err
) {
    adder_box_tuple_t tuple;
    adder_box_error_t tmp_err;

    if (!getter(index, key_or_rnd, &tuple, &tmp_err)) {
        *err = tmp_err;
        return false;
    }

    bool ok = tarantool_tarantool_box_tuple_to_buf(&tuple, ret, &tmp_err);
    if (!ok) {
        *err = tmp_err;
        return false;
    }

    return true;
}

bool exports_adder_t_random(adder_index_t *index, uint32_t rnd, adder_list_u8_t *ret, adder_box_error_t *err) {
    return index_to_buf((bool (*)(adder_index_t *, void *, adder_box_tuple_t *, adder_box_error_t *))
        tarantool_tarantool_index_random, index, &rnd, ret, err);
}

bool exports_adder_t_get(adder_index_t *index, adder_list_u8_t *key, adder_list_u8_t *ret, adder_box_error_t *err) {
    return index_to_buf((bool (*)(adder_index_t *, void *, adder_box_tuple_t *, adder_box_error_t *))
        tarantool_tarantool_index_get, index, key, ret, err);
}

bool exports_adder_t_min(adder_index_t *index, adder_list_u8_t *key, adder_list_u8_t *ret, adder_box_error_t *err) {
    return index_to_buf((bool (*)(adder_index_t *, void *, adder_box_tuple_t *, adder_box_error_t *))
        tarantool_tarantool_index_min, index, key, ret, err);
}

bool exports_adder_t_max(adder_index_t *index, adder_list_u8_t *key, adder_list_u8_t *ret, adder_box_error_t *err) {
    return index_to_buf((bool (*)(adder_index_t *, void *, adder_box_tuple_t *, adder_box_error_t *))
        tarantool_tarantool_index_max, index, key, ret, err);
}

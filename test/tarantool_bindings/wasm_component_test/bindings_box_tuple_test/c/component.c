#include "bindings/adder.h"

#include <stdlib.h>
#include <string.h>

bool exports_adder_t_new(adder_msgpack_value_t *data, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_new(data, ret, err);
}

void exports_adder_t_retain(adder_box_tuple_t *t) {
    tarantool_tarantool_box_tuple_retain(t);
}

void exports_adder_t_release(adder_box_tuple_t *t) {
    tarantool_tarantool_box_tuple_release(t);
}

uint32_t exports_adder_t_field_count(adder_box_tuple_t *t) {
    return tarantool_tarantool_box_tuple_field_count(t);
}

uint64_t exports_adder_t_byte_size(adder_box_tuple_t *t) {
    return tarantool_tarantool_box_tuple_byte_size(t);
}

bool exports_adder_t_to_bytes(adder_box_tuple_t *t, adder_msgpack_value_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_to_bytes(t, ret, err);
}

void exports_adder_t_format(adder_box_tuple_t *t, adder_tuple_format_t *ret) {
    tarantool_tarantool_box_tuple_format(t, ret);
}

bool exports_adder_t_field(adder_box_tuple_t *t, uint32_t idx, adder_option_tuple_field_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_field(t, idx, ret, err);
}

bool exports_adder_t_field_by_path(adder_box_tuple_t *t, adder_string_t *path, adder_option_tuple_field_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_field_by_path(t, path, ret, err);
}

bool exports_adder_t_update(adder_box_tuple_t *t, adder_msgpack_value_t *expr, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_update(t, expr, ret, err);
}

bool exports_adder_t_upsert(adder_box_tuple_t *t, adder_msgpack_value_t *expr, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_upsert(t, expr, ret, err);
}

bool exports_adder_t_validate(adder_box_tuple_t *t, adder_tuple_format_t *format, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_validate(t, format, err);
}

int32_t exports_adder_t_compare(adder_key_def_t *key_def, adder_box_tuple_t *left, adder_box_tuple_t *right) {
    return tarantool_tarantool_box_tuple_compare(key_def, left, right);
}

bool exports_adder_t_compare_with_key(adder_box_tuple_t *tuple, adder_msgpack_value_t *key, adder_key_def_t *key_def, int32_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_compare_with_key(tuple, key, key_def, ret, err);
}

bool exports_adder_t_key_def_new(adder_list_key_part_t *parts, adder_key_def_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_new(
        (tarantool_tarantool_key_def_list_key_part_t *)parts,
        ret,
        err);
}

void exports_adder_t_tuple_format_default(adder_tuple_format_t *ret) {
    tarantool_tarantool_tuple_format_default(ret);
}

bool exports_adder_t_tuple_format_new(adder_list_key_def_t *key_defs, adder_tuple_format_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_tuple_format_new(
        (tarantool_tarantool_tuple_format_list_key_def_t *)key_defs,
        ret,
        err);
}

static bool append_bytes(uint8_t *buffer, size_t capacity, size_t *offset, const uint8_t *data, size_t len) {
    if (len > capacity - *offset) {
        return false;
    }
    memcpy(buffer + *offset, data, len);
    *offset += len;
    return true;
}

static bool append_u32(uint8_t *buffer, size_t capacity, size_t *offset, uint32_t value) {
    if (value <= 0x7f) {
        uint8_t encoded = (uint8_t)value;
        return append_bytes(buffer, capacity, offset, &encoded, 1);
    }
    uint8_t encoded[5] = {
        0xce,
        (uint8_t)(value >> 24),
        (uint8_t)(value >> 16),
        (uint8_t)(value >> 8),
        (uint8_t)value,
    };
    return append_bytes(buffer, capacity, offset, encoded, sizeof(encoded));
}

static bool append_field(
    uint8_t *buffer,
    size_t capacity,
    size_t *offset,
    adder_option_tuple_field_t *field) {
    if (!field->is_some) {
        return false;
    }
    return append_bytes(buffer, capacity, offset, field->val.ptr, field->val.len);
}

bool exports_adder_t_iter_test_all(adder_box_tuple_t *t, adder_msgpack_value_t *ret, adder_box_error_t *err) {
    tarantool_tarantool_box_tuple_own_iterator_t owned;
    if (!tarantool_tarantool_box_tuple_static_iterator_new(t, &owned, err)) {
        return false;
    }
    tarantool_tarantool_box_tuple_borrow_iterator_t iterator = {
        .__handle = owned.__handle,
    };

    size_t capacity = (size_t)tarantool_tarantool_box_tuple_byte_size(t) * 3
        + (size_t)tarantool_tarantool_box_tuple_field_count(t) * 12 + 128;
    uint8_t *buffer = malloc(capacity);
    if (buffer == NULL) {
        tarantool_tarantool_box_tuple_iterator_drop_own(owned);
        return false;
    }

    size_t offset = 5;
    uint32_t item_count = 0;
    bool ok = true;

    for (;;) {
        uint32_t position = tarantool_tarantool_box_tuple_method_iterator_position(iterator);
        adder_option_tuple_field_t field = {0};
        if (!tarantool_tarantool_box_tuple_method_iterator_next(iterator, &field, err)) {
            ok = false;
            break;
        }
        if (!field.is_some) {
            break;
        }
        uint8_t pair = 0x92;
        ok = append_bytes(buffer, capacity, &offset, &pair, 1)
            && append_u32(buffer, capacity, &offset, position)
            && append_field(buffer, capacity, &offset, &field);
        adder_option_tuple_field_free(&field);
        if (!ok) {
            break;
        }
        item_count++;
    }

    if (ok) {
        tarantool_tarantool_box_tuple_method_iterator_rewind(iterator);
        uint32_t position = tarantool_tarantool_box_tuple_method_iterator_position(iterator);
        adder_option_tuple_field_t field = {0};
        ok = tarantool_tarantool_box_tuple_method_iterator_next(iterator, &field, err);
        if (ok) {
            const uint8_t prefix[] = {0x93, 0xa6, 'r', 'e', 'w', 'i', 'n', 'd'};
            ok = append_bytes(buffer, capacity, &offset, prefix, sizeof(prefix))
                && append_u32(buffer, capacity, &offset, position)
                && append_field(buffer, capacity, &offset, &field);
            adder_option_tuple_field_free(&field);
            if (ok) {
                item_count++;
            }
        }
    }

    if (ok) {
        adder_option_tuple_field_t field = {0};
        ok = tarantool_tarantool_box_tuple_method_iterator_seek(iterator, 1, &field, err);
        if (ok) {
            uint32_t position = tarantool_tarantool_box_tuple_method_iterator_position(iterator);
            const uint8_t prefix[] = {0x93, 0xa4, 's', 'e', 'e', 'k'};
            ok = append_bytes(buffer, capacity, &offset, prefix, sizeof(prefix))
                && append_u32(buffer, capacity, &offset, position)
                && append_field(buffer, capacity, &offset, &field);
            adder_option_tuple_field_free(&field);
            if (ok) {
                item_count++;
            }
        }
    }

    tarantool_tarantool_box_tuple_iterator_drop_own(owned);
    if (!ok) {
        free(buffer);
        return false;
    }

    buffer[0] = 0xdd;
    buffer[1] = (uint8_t)(item_count >> 24);
    buffer[2] = (uint8_t)(item_count >> 16);
    buffer[3] = (uint8_t)(item_count >> 8);
    buffer[4] = (uint8_t)item_count;
    ret->ptr = buffer;
    ret->len = offset;
    return true;
}

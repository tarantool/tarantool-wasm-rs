#include "adder.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

bool exports_adder_t_new(adder_list_u8_t *data, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_new(data, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

int32_t exports_adder_t_ref(adder_box_tuple_t *t) {
    return tarantool_tarantool_box_tuple_ref(t);
}

void exports_adder_t_unref(adder_box_tuple_t *t) {
    return tarantool_tarantool_box_tuple_unref(t);
}

uint32_t exports_adder_t_field_count(adder_box_tuple_t *t) {
    return tarantool_tarantool_box_tuple_field_count(t);
}

uint64_t exports_adder_t_bsize(adder_box_tuple_t *t) {
    return tarantool_tarantool_box_tuple_bsize(t);
}

bool exports_adder_t_to_buf(adder_box_tuple_t *t, adder_list_u8_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_to_buf(t, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

void exports_adder_t_format(adder_box_tuple_t *t, adder_tuple_format_t *ret) {
    return tarantool_tarantool_box_tuple_format(t, ret);
}

bool exports_adder_t_field(adder_box_tuple_t *t, uint32_t idx, adder_tuple_field_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_field(t, idx, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

bool exports_adder_t_field_by_path(adder_box_tuple_t *t, adder_string_t *path, int32_t index_base, adder_tuple_field_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_field_by_path(t, path, index_base, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

bool exports_adder_t_update(adder_box_tuple_t *t, adder_list_u8_t *expr, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_update(t, expr, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

bool exports_adder_t_upsert(adder_box_tuple_t *t, adder_list_u8_t *expr, adder_box_tuple_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_box_tuple_upsert(t, expr, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

bool exports_adder_t_validate(adder_box_tuple_t *t, adder_tuple_format_t *format) {
    return tarantool_tarantool_box_tuple_validate(t, format);
}

int32_t exports_adder_t_compare(adder_key_def_t *key_def, adder_box_tuple_t *left, adder_box_tuple_t *right) {
    return tarantool_tarantool_box_tuple_compare(key_def, left, right);
}

int32_t exports_adder_t_compare_with_key(adder_box_tuple_t *tuple, adder_list_u8_t *key, adder_key_def_t *key_def) {
    return tarantool_tarantool_box_tuple_compare_with_key(tuple, key, key_def);
}

bool exports_adder_t_key_def_new(adder_list_key_part_t *parts, adder_key_def_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_key_def_new((tarantool_tarantool_key_def_list_key_part_t *)parts, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

void exports_adder_t_tuple_format_default(adder_tuple_format_t *ret) {
    return tarantool_tarantool_tuple_format_default(ret);
}

bool exports_adder_t_tuple_format_new(adder_list_key_def_t *key_defs, adder_tuple_format_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_tuple_format_new(key_defs, ret, (tarantool_tarantool_box_tuple_box_error_t *) err);
}

bool exports_adder_t_decode_from_raw_ptr(uint64_t ptr, adder_list_u8_t *ret, adder_box_error_t *err) {
    return exports_adder_t_decode_from_raw_ptr(ptr, ret, err);
}

/*
MessagePack is encoded manually because C lacks native JSON support.
Using tarantool_msgpack_encode would require constructing a JSON string,
which is complex and error-prone in C. Manual encoding avoids this overhead.
*/
bool exports_adder_t_iter_test_all(adder_box_tuple_t *t, adder_list_u8_t *ret, adder_box_error_t *err) {

    tarantool_tarantool_box_tuple_own_tuple_iterator_t iter;
    tarantool_tarantool_box_tuple_box_error_t iter_err;

    if (!tarantool_tarantool_box_tuple_static_tuple_iterator_new_tuple_iterator(t, &iter, &iter_err)) {
        *err = iter_err;
        return false;
    }

    tarantool_tarantool_box_tuple_borrow_tuple_iterator_t biter =
        tarantool_tarantool_box_tuple_borrow_tuple_iterator(iter);

    uint8_t buf[4096];
    size_t offset = 0;

    // msgpack array header (we'll fix size later)
    buf[offset++] = 0x90; // empty array for now

    uint32_t count = 0;

    for (;;) {
        uint32_t pos = tarantool_tarantool_box_tuple_method_tuple_iterator_position(biter);
        tarantool_tarantool_box_tuple_tuple_field_t field;
        tarantool_tarantool_box_tuple_box_error_t field_err;

        if (!tarantool_tarantool_box_tuple_method_tuple_iterator_next(biter, &field, &field_err)) {
            break;
        }
        if (field.ptr == 0)
            break;

        // [pos, field] as fixarray 2
        if (offset + 1 + 5 + 9 > sizeof(buf)) break; // safety
        buf[offset++] = 0x92;                    // array of 2
        buf[offset++] = 0xcc; buf[offset++] = pos; // uint8
        buf[offset++] = 0xcf; // uint64
        for (int i = 7; i >= 0; i--)
            buf[offset++] = (field.ptr >> (i * 8)) & 0xFF;
        count++;
    }

    // rewind & next
    tarantool_tarantool_box_tuple_method_tuple_iterator_rewind(biter);
    uint32_t rpos = tarantool_tarantool_box_tuple_method_tuple_iterator_position(biter);
    tarantool_tarantool_box_tuple_tuple_field_t rfield;
    tarantool_tarantool_box_tuple_box_error_t rerr;
    tarantool_tarantool_box_tuple_method_tuple_iterator_next(biter, &rfield, &rerr);

    buf[offset++] = 0x93; // array of 3: "rewind", rpos, rfield
    buf[offset++] = 0xa6; memcpy(&buf[offset], "rewind", 6); offset += 6;
    buf[offset++] = 0xcc; buf[offset++] = rpos;
    buf[offset++] = 0xcf;
    for (int i = 7; i >= 0; i--)
        buf[offset++] = (rfield.ptr >> (i * 8)) & 0xFF;
    count++;

    // seek(1)
    tarantool_tarantool_box_tuple_tuple_field_t sfield;
    tarantool_tarantool_box_tuple_box_error_t serr;
    tarantool_tarantool_box_tuple_method_tuple_iterator_seek(biter, 1, &sfield, &serr);
    uint32_t spos = tarantool_tarantool_box_tuple_method_tuple_iterator_position(biter);

    buf[offset++] = 0x93; // array of 3: "seek", spos, sfield
    buf[offset++] = 0xa4; memcpy(&buf[offset], "seek", 4); offset += 4;
    buf[offset++] = 0xcc; buf[offset++] = spos;
    buf[offset++] = 0xcf;
    for (int i = 7; i >= 0; i--)
        buf[offset++] = (sfield.ptr >> (i * 8)) & 0xFF;
    count++;

    // Fix top-level array length
    if (count <= 15)
        buf[0] = 0x90 + count;
    else if (count <= 0xffff) {
        memmove(&buf[3], &buf[1], offset - 1);
        buf[0] = 0xdc;
        buf[1] = (count >> 8) & 0xff;
        buf[2] = count & 0xff;
        offset += 2;
    } else {
        // unlikely
        return false;
    }

    // Wrap as list<u8>
    adder_list_u8_t raw;
    raw.ptr = malloc(offset);
    if (!raw.ptr) return false;
    memcpy(raw.ptr, buf, offset);
    raw.len = offset;
    *ret = raw;
    return true;
}

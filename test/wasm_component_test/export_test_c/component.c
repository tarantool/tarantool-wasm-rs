#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include "adder.h"

bool exports_adder_t_bool(bool x) {
    return x;
}

int8_t exports_adder_t_s8(int8_t x) {
    return x;
}

int16_t exports_adder_t_s16(int16_t x) {
    return x;
}

int32_t exports_adder_t_s32(int32_t x) {
    return x;
}

int64_t exports_adder_t_s64(int64_t x) {
    return x;
}

uint8_t exports_adder_t_u8(uint8_t x) {
    return x;
}

uint16_t exports_adder_t_u16(uint16_t x) {
    return x;
}

uint32_t exports_adder_t_u32(uint32_t x) {
    return x;
}

uint64_t exports_adder_t_u64(uint64_t x) {
    return x;
}

float exports_adder_t_f32(float x) {
    return x;
}

double exports_adder_t_f64(double x) {
    return x;
}

uint32_t exports_adder_t_char(uint32_t x) {
    return x;
}

void exports_adder_t_string(adder_string_t *x, adder_string_t *ret) {
    ret->len = x->len;
    ret->ptr = malloc(x->len);
    memcpy(ret->ptr, x->ptr, x->len);
}

void exports_adder_t_list(adder_list_string_t *x, adder_list_string_t *ret) {
    ret->len = x->len;
    ret->ptr = malloc(x->len * sizeof(adder_string_t));
    for (size_t i = 0; i < x->len; ++i) {
        ret->ptr[i].len = x->ptr[i].len;
        ret->ptr[i].ptr = malloc(x->ptr[i].len);
        memcpy(ret->ptr[i].ptr, x->ptr[i].ptr, x->ptr[i].len);
    }
}

bool exports_adder_t_option(uint8_t *maybe_x, uint8_t *ret) {
    *ret = *maybe_x;
    return true;
}

bool exports_adder_t_result(bool x, bool *ret, adder_string_t *err) {
    if (x) {
        *ret = x;
        return true;
    } else {
        const char *msg = "error";
        err->len = strlen(msg);
        err->ptr = malloc(err->len);
        memcpy(err->ptr, msg, err->len);
        return false;
    }
}

void exports_adder_t_tuple(adder_tuple3_bool_f32_string_t *x, adder_tuple2_bool_string_t *ret) {
    ret->f0 = x->f0;
    ret->f1.len = x->f2.len;
    ret->f1.ptr = malloc(x->f2.len);
    memcpy(ret->f1.ptr, x->f2.ptr, x->f2.len);
}

void exports_adder_t_record(adder_r_t *x, adder_r_t *ret) {
    ret->x = x->x;
    ret->y.len = x->y.len;
    ret->y.ptr = malloc(x->y.len);
    memcpy(ret->y.ptr, x->y.ptr, x->y.len);
}

void exports_adder_t_variant(adder_v_t *x, adder_v_t *ret) {
    ret->tag = x->tag;
    switch (x->tag) {
        case ADDER_V_ANY:
            ret->val.any = x->val.any;
            break;
        case ADDER_V_RESTRICTED:
            ret->val.restricted.len = x->val.restricted.len;
            ret->val.restricted.ptr = malloc(x->val.restricted.len * sizeof(adder_t_t));
            for (size_t i = 0; i < x->val.restricted.len; ++i) {
                ret->val.restricted.ptr[i].len = x->val.restricted.ptr[i].len;
                ret->val.restricted.ptr[i].ptr = malloc(x->val.restricted.ptr[i].len);
                memcpy(ret->val.restricted.ptr[i].ptr,
                       x->val.restricted.ptr[i].ptr,
                       x->val.restricted.ptr[i].len);
            }
            break;
        default:
            break;
    }
}

adder_e_t exports_adder_t_enum(adder_e_t x) {
    return x;
}

adder_f_t exports_adder_t_flags(adder_f_t x) {
    return x;
}
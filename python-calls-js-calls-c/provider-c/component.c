#include "cprovider.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void log_info(const char *msg) {
    cprovider_string_t str = {
        .ptr = (uint8_t *)msg,
        .len = strlen(msg),
    };
    tarantool_tarantool_say_say_info(&str, NULL);
}
int32_t exports_demo_cross_call_coremath_add_base(int32_t left, int32_t right) {
    char buf[128];
    snprintf(buf, sizeof(buf), "c provider: add-base(%d, %d)", left, right);
    log_info(buf);
    return left + right;
}

void exports_demo_cross_call_coremath_label_base(int32_t left, int32_t right, cprovider_string_t *ret) {
    char buf[128];
    snprintf(buf, sizeof(buf), "c provider: label-base(%d, %d)", left, right);
    log_info(buf);

    int total = left + right;
    int needed = snprintf(NULL, 0, "c says: %d + %d = %d", left, right, total);
    ret->ptr = malloc((size_t)needed + 1);
    ret->len = (size_t)needed;
    snprintf((char *)ret->ptr, (size_t)needed + 1, "c says: %d + %d = %d", left, right, total);
}

#include "adder.h"

void exports_adder_t_error_new(adder_string_t *message,
                               adder_string_t *type,
                               uint32_t code,
                               adder_string_t *maybe_file,
                               uint32_t *maybe_line,
                               adder_box_error_t *ret) {
    tarantool_tarantool_error_error_new(message, type, code, maybe_file, maybe_line, ret);
}

void exports_adder_t_set(adder_box_error_t *err_) {
    tarantool_tarantool_error_set(err_);
}

bool exports_adder_t_last(adder_box_error_t *ret) {
    return tarantool_tarantool_error_last(ret);
}

void exports_adder_t_clear(void) {
    tarantool_tarantool_error_clear();
}

void exports_adder_t_to_string(adder_box_error_t *err_, adder_string_t *ret) {
    tarantool_tarantool_error_to_string(err_, ret);
}

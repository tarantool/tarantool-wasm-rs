#include "bindings/adder.h"

void exports_adder_t_new(adder_error_code_t code, adder_string_t *message, adder_box_error_t *ret) {
    tarantool_tarantool_error_new(code, message, ret);
}

void exports_adder_t_new_with_location(adder_error_code_t code, adder_string_t *message, adder_string_t *maybe_file, uint32_t *maybe_line, adder_box_error_t *ret) {
    tarantool_tarantool_error_new_with_location(code, message, maybe_file, maybe_line, ret);
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

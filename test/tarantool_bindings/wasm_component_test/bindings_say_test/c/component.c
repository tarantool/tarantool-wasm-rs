#include "bindings/adder.h"

static void write(adder_log_level_t level,
                  adder_string_t *message,
                  adder_log_context_t *context) {
    tarantool_tarantool_log_write(level, message, context);
}

void exports_adder_t_say(adder_log_level_t level,
                         adder_string_t *message,
                         adder_log_context_t *context) {
    write(level, message, context);
}

void exports_adder_t_say_error(adder_string_t *message,
                               adder_log_context_t *context) {
    write(TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_ERROR, message, context);
}

void exports_adder_t_say_crit(adder_string_t *message,
                              adder_log_context_t *context) {
    write(TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_CRITICAL, message, context);
}

void exports_adder_t_say_warn(adder_string_t *message,
                              adder_log_context_t *context) {
    write(TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_WARNING, message, context);
}

void exports_adder_t_say_info(adder_string_t *message,
                              adder_log_context_t *context) {
    write(TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_INFO, message, context);
}

void exports_adder_t_say_verbose(adder_string_t *message,
                                 adder_log_context_t *context) {
    write(TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_VERBOSE, message, context);
}

void exports_adder_t_say_debug(adder_string_t *message,
                               adder_log_context_t *context) {
    write(TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_DEBUG, message, context);
}

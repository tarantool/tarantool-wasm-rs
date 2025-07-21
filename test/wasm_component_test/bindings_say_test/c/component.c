#include "adder.h"

void exports_adder_t_say(adder_log_level_t level, adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say((tarantool_tarantool_say_log_level_t) level, (adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_error(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_error((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_crit(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_crit((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_warn(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_warn((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_info(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_info((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_verbose(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_verbose((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_debug(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_debug((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

void exports_adder_t_say_syserror(adder_string_t *msg, adder_log_context_t *maybe_ctx) {
    tarantool_tarantool_say_say_syserror((adder_string_t *) msg, (tarantool_tarantool_say_option_log_context_t *) maybe_ctx);
}

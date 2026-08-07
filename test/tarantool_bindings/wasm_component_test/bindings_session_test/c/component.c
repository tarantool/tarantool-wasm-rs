#include "bindings/adder.h"

void exports_adder_t_current(adder_session_t *ret) {
    tarantool_tarantool_session_current((tarantool_tarantool_session_session_t *) ret);
}

bool exports_adder_t_broadcast(adder_string_t *key, adder_string_t *value, adder_box_error_t *err) {
    return tarantool_tarantool_session_broadcast(key, value, err);
}

bool exports_adder_t_iproto_send(adder_session_t *session,
                                 adder_msgpack_value_t *header,
                                 adder_msgpack_value_t *maybe_body,
                                 adder_box_error_t *err) {
    return tarantool_tarantool_session_iproto_send(session, header,
                                                   maybe_body, err);
}

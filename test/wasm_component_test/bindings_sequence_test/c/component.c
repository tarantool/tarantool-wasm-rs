#include "adder.h"

bool exports_adder_t_next(adder_sequence_t *seq, int64_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_sequence_next((tarantool_tarantool_sequence_sequence_t *)seq, ret, (tarantool_tarantool_sequence_box_error_t *)err);
}

bool exports_adder_t_current(adder_sequence_t *seq, int64_t *ret, adder_box_error_t *err) {
    return tarantool_tarantool_sequence_current((tarantool_tarantool_sequence_sequence_t *)seq, ret, (tarantool_tarantool_sequence_box_error_t *)err);
}

bool exports_adder_t_set(adder_sequence_t *seq, int64_t value, adder_box_error_t *err) {
    return tarantool_tarantool_sequence_set((tarantool_tarantool_sequence_sequence_t *)seq, value, (tarantool_tarantool_sequence_box_error_t *)err);
}

bool exports_adder_t_reset(adder_sequence_t *seq, adder_box_error_t *err) {
    return tarantool_tarantool_sequence_reset((tarantool_tarantool_sequence_sequence_t *)seq, (tarantool_tarantool_sequence_box_error_t *)err);
}

#include "adder.h"
#include <stdio.h>
#include <string.h>
#include <inttypes.h>

static void log_info(const char *msg) {
    adder_string_t str = {
        .ptr = (uint8_t *)msg,
        .len = strlen(msg)
    };
    tarantool_tarantool_say_say_info(&str, NULL);
}

static const char *isolation_to_str(tarantool_tarantool_txn_txn_isolation_level_t level) {
    switch (level) {
        case TARANTOOL_TARANTOOL_TYPES_TXN_ISOLATION_LEVEL_TXN_ISOLATION_DEFAULT:
            return "TXN_ISOLATION_DEFAULT";
        case TARANTOOL_TARANTOOL_TYPES_TXN_ISOLATION_LEVEL_TXN_ISOLATION_BEST_EFFORT:
            return "TXN_ISOLATION_BEST_EFFORT";
        case TARANTOOL_TARANTOOL_TYPES_TXN_ISOLATION_LEVEL_TXN_ISOLATION_READ_COMMITTED:
            return "TXN_ISOLATION_READ_COMMITTED";
        case TARANTOOL_TARANTOOL_TYPES_TXN_ISOLATION_LEVEL_TXN_ISOLATION_READ_CONFIRMED:
            return "TXN_ISOLATION_READ_CONFIRMED";
        case TARANTOOL_TARANTOOL_TYPES_TXN_ISOLATION_LEVEL_TXN_ISOLATION_LINEARIZABLE:
            return "TXN_ISOLATION_LINEARIZABLE";
        default:
            return "UNKNOWN_ISOLATION";
    }
}

bool exports_wasi_cli_run_run(void) {
    char buf[128];
    tarantool_tarantool_txn_box_error_t err;

    snprintf(buf, sizeof(buf), "txn() before begin: %s", tarantool_tarantool_txn_txn() ? "True" : "False");
    log_info(buf);

    if (!tarantool_tarantool_txn_begin(&err)) return false;
    log_info("txn.begin()");

    int64_t txn_id = tarantool_tarantool_txn_txn_id();
    tarantool_tarantool_txn_txn_isolation_level_t isolation = tarantool_tarantool_txn_txn_isolation();

    snprintf(buf, sizeof(buf), "txn_id first: %" PRId64, txn_id);
    log_info(buf);

    snprintf(buf, sizeof(buf), "txn_isolation: %s", isolation_to_str(isolation));
    log_info(buf);

    tarantool_tarantool_txn_txn_make_sync();
    log_info("txn_make_sync()");

    tarantool_tarantool_txn_txn_set_isolation(
        TARANTOOL_TARANTOOL_TYPES_TXN_ISOLATION_LEVEL_TXN_ISOLATION_BEST_EFFORT,
        &err
    );

    isolation = tarantool_tarantool_txn_txn_isolation();
    snprintf(buf, sizeof(buf), "txn_isolation after set: %s", isolation_to_str(isolation));
    log_info(buf);

    if (!tarantool_tarantool_txn_commit(&err)) return false;
    log_info("txn.commit()");

    if (!tarantool_tarantool_txn_begin(&err)) return false;
    log_info("txn.begin() second");

    snprintf(buf, sizeof(buf), "txn_id second: %" PRId64, txn_id);
    log_info(buf);

    if (!tarantool_tarantool_txn_rollback(&err)) return false;
    log_info("txn.rollback()");

    snprintf(buf, sizeof(buf), "txn() after rollback: %s", tarantool_tarantool_txn_txn() ? "True" : "False");
    log_info(buf);

    return true;
}

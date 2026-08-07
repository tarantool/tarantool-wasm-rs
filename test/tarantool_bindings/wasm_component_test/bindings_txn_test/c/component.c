#include "bindings/adder.h"
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

static void log_info(const char *message) {
    adder_string_t text = {
        .ptr = (uint8_t *)message,
        .len = strlen(message),
    };
    tarantool_tarantool_log_write(
        TARANTOOL_TARANTOOL_TYPES_LOG_LEVEL_INFO, &text, NULL);
}

static const char *isolation_to_string(
    tarantool_tarantool_transaction_transaction_isolation_t isolation) {
    switch (isolation) {
    case TARANTOOL_TARANTOOL_TYPES_TRANSACTION_ISOLATION_DEFAULT:
        return "default";
    case TARANTOOL_TARANTOOL_TYPES_TRANSACTION_ISOLATION_BEST_EFFORT:
        return "best-effort";
    case TARANTOOL_TARANTOOL_TYPES_TRANSACTION_ISOLATION_READ_COMMITTED:
        return "read-committed";
    case TARANTOOL_TARANTOOL_TYPES_TRANSACTION_ISOLATION_READ_CONFIRMED:
        return "read-confirmed";
    case TARANTOOL_TARANTOOL_TYPES_TRANSACTION_ISOLATION_LINEARIZABLE:
        return "linearizable";
    default:
        return "unknown";
    }
}

static void log_id(const char *label) {
    char message[128];
    uint64_t id;
    if (tarantool_tarantool_transaction_id(&id)) {
        snprintf(message, sizeof(message), "%s: %" PRIu64, label, id);
    } else {
        snprintf(message, sizeof(message), "%s: none", label);
    }
    log_info(message);
}

static void log_isolation(const char *label) {
    char message[128];
    tarantool_tarantool_transaction_transaction_isolation_t isolation;
    if (tarantool_tarantool_transaction_isolation(&isolation)) {
        snprintf(message, sizeof(message), "%s: %s", label,
                 isolation_to_string(isolation));
    } else {
        snprintf(message, sizeof(message), "%s: none", label);
    }
    log_info(message);
}

bool exports_wasi_cli_run_run(void) {
    tarantool_tarantool_transaction_box_error_t error;

    log_id("transaction id before begin");
    if (!tarantool_tarantool_transaction_begin(&error)) {
        return false;
    }
    log_info("transaction.begin()");
    log_id("transaction id first");
    log_isolation("transaction isolation");

    tarantool_tarantool_transaction_make_synchronous();
    log_info("transaction.make_synchronous()");

    if (!tarantool_tarantool_transaction_set_isolation(
            TARANTOOL_TARANTOOL_TYPES_TRANSACTION_ISOLATION_BEST_EFFORT,
            &error)) {
        return false;
    }
    log_info("transaction.set_isolation()");
    log_isolation("transaction isolation after set");

    if (!tarantool_tarantool_transaction_commit(&error)) {
        return false;
    }
    log_info("transaction.commit()");

    if (!tarantool_tarantool_transaction_begin(&error)) {
        return false;
    }
    log_info("transaction.begin() second");
    log_id("transaction id second");

    if (!tarantool_tarantool_transaction_rollback(&error)) {
        return false;
    }
    log_info("transaction.rollback()");
    log_id("transaction id after rollback");
    return true;
}

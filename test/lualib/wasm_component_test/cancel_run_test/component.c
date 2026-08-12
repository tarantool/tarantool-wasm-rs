#include "bindings/adder.h"

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

bool exports_wasi_cli_run_run(void) {
    const char *exit_code = getenv("EXIT_CODE");
    if (exit_code != NULL) {
        exit(atoi(exit_code));
    }
    if (getenv("TRAP") != NULL) {
        __builtin_trap();
    }

    if (getenv("INF") != NULL) {
        for (;;) {
            puts("1");
            fflush(stdout);
            usleep(100000);
        }
    }

    puts("2");
    return true;
}

#include "bindings/adder.h"

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

bool exports_wasi_cli_run_run(void) {
    if (getenv("PIPE_ECHO") != NULL) {
        if (getenv("PIPE_DELAY") != NULL) {
            sleep(1);
        }
        unsigned char buffer[4096];
        size_t size;
        while ((size = fread(buffer, 1, sizeof(buffer), stdin)) != 0) {
            fwrite(buffer, 1, size, stdout);
            if (getenv("PIPE_STDERR") != NULL) {
                fwrite(buffer, 1, size, stderr);
            }
            fflush(stdout);
            fflush(stderr);
        }
        return true;
    }
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

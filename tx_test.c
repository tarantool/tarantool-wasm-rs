#include <stdio.h>
#include <string.h>
#include <tarantool/module.h>
#include <tarantool/lua.h>
#include <tarantool/lauxlib.h>
#include <stdarg.h>
#include <pthread.h>
#include <stdatomic.h>
#include <unistd.h>

static atomic_int task1_done = 0;

void log_box_error(const char *msg) {
    box_error_t *err = box_error_last();
    if (err != NULL) {
        say_error("%s: %s", msg, box_error_message(err));
    }
}

void tx_task1(void *arg) {
    say_info("Task 1 started");

    if (box_txn_begin() != 0) {
        log_box_error("Failed to start transaction");
        return;
    }

    uint32_t space_id = box_space_id_by_name("test_space", 10);
    if (space_id == BOX_ID_NIL) {
        log_box_error("Space not found");
        box_txn_rollback();
        return;
    }

    const char tuple_data[] = { 0x92, 0x01, 0xa5, 'h', 'e', 'l', 'l', 'o' };
    const char *end = tuple_data + sizeof(tuple_data);

    int res = box_insert(space_id, tuple_data, end, NULL);
    if (res == -1) {
        log_box_error("Insert failed");
        box_txn_rollback();
        return;
    }

    say_info("Task 1 INSERT success");

    if (box_txn_commit() == -1) {
        log_box_error("Commit failed");
        box_txn_rollback();
    }

    atomic_store(&task1_done, 1);
    say_info("Task 1 completed");
}

void tx_task2(void *arg) {
    say_info("Task 2 started");

    if (box_txn_begin() != 0) {
        log_box_error("Failed to start transaction");
        return;
    }

    uint32_t space_id = box_space_id_by_name("test_space", 10);
    if (space_id == BOX_ID_NIL) {
        log_box_error("Space not found");
        box_txn_rollback();
        return;
    }

    uint32_t index_id = 0;
    const char key_data[] = { 0x91, 0x01 };
    const char *key_end = key_data + sizeof(key_data);
    const char ops_data[] = { 0x91, 0x93, 0xa1, '=', 0x02, 0xa5, 'w', 'o', 'r', 'l', 'd' };
    const char *ops_end = ops_data + sizeof(ops_data);

    int res = box_update(space_id, index_id, key_data, key_end, ops_data, ops_end, 1, NULL);
    if (res == -1) {
        log_box_error("Update failed");
        box_txn_rollback();
        return;
    }

    say_info("Task 2 UPDATE success");

    if (box_txn_commit() == -1) {
        log_box_error("Commit failed");
        box_txn_rollback();
    }

    atomic_store(&task1_done, 2);
    say_info("Task 2 completed");
}

void* run_tx_tasks(void *arg) {
    say_info("Running TX tasks in pthread");

    atomic_store(&task1_done, 0);

    tnt_tx_push(tx_task1, NULL);
    say_info("Task 1 pushed");
    
    tnt_tx_flush();
    say_info("Task 1 flushed");

    while (atomic_load(&task1_done) == 0) {
        usleep(100000);
    }

    tnt_tx_push(tx_task2, NULL);
    say_info("Task 2 pushed");

    tnt_tx_flush();
    say_info("Task 2 flushed");

    return NULL;
}

static int l_run_tx(lua_State *L) {
    pthread_t thread;
    int res = pthread_create(&thread, NULL, run_tx_tasks, NULL);
    if (res != 0) {
        say_error("Failed to create thread: %s", strerror(res));
        return 0;
    }

    pthread_join(thread, NULL);

    say_info("All TX tasks completed");
    return 0;
}

static int l_join_tx(lua_State *L) {
    while (atomic_load(&task1_done) != 2) {
        usleep(100000);
    }
    return 0;
}

int luaopen_tx_test(lua_State *L) {
    static const struct luaL_Reg lib[] = {
        {"run", l_run_tx},
        {"join", l_join_tx},
        {NULL, NULL},
    };
    luaL_register(L, "tx_test", lib);
    return 1;
}

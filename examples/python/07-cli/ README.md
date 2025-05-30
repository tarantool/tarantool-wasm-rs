# Python CLI inside Tarantool

**Running the WASM module adder.wasm using Tarantool:**
```sh
$ tarantool run.lua adder.wasm
```

**Tarantool logs during startup:**
```
2025-05-30 12:06:54.356 [48252] main/104/run.lua main.cc:496 I> Tarantool 3.4.0-entrypoint-225-g9ad1bd0ab0 Linux-x86_64-Debug
2025-05-30 12:06:54.356 [48252] main/104/run.lua main.cc:499 I> log level 6 (VERBOSE)
<...snipped...>
2025-05-30 12:06:54.406 [48252] main/104/run.lua/box.feedback_daemon feedback_daemon.lua:491 V> feedback_daemon started
117
1. !!!
2025-05-30 12:07:24.152 [48252] main/106/checkpoint_daemon gc.c:643 I> scheduled next checkpoint for Fri May 30 13:30:34 2025
Python 3.12.1 (118e9d8:118e9d8, May 16 2025, 14:40:11) [Clang 18.1.0rc (https://github.com/llvm/llvm-project 461274b81d8641eab64d494acc on wasi
Type "help", "copyright", "credits" or "license" for more information.
(InteractiveConsole)
>>>    # Entering Python interactive console embedded in Tarantool
```

**Checking fiber information via Lua calls from Python:**
```
>>> print(wlua.call("require('fiber').info()"))
--- {}
```

**Start a transaction and check fiber information again:**
```
>>> txn.begin()
>>> print(wlua.call("require('fiber').info()"))
<...snipped...>
117:
  backtrace:
  - C: '#1  0x5c56ff4dd6f0 in fiber_yield+18'
  - C: '#2  0x5c56ff4dd95d in fiber_yield_timeout+314'
  - C: '#3  0x5c56ff4ddb64 in fiber_sleep+244'
  - C: '#4  0x5c56ff4a330c in lbox_fiber_sleep+280'
  - L: '#5  sleep in =[C]:-1'
  - L: '#6  (unnamed) in @run.lua:19'
  - C: '#7  0x5c56ff54fbfc in lua_pcall+968'
  - C: '#8  0x5c56ff4ad720 in luaT_call+45'
  - C: '#9  0x5c56ff4a24c4 in lua_fiber_run_f+189'
  - C: '#10 0x5c56ff1f3137 in fiber_cxx_invoke(int (*)(__va_list_tag*), __va_list_tag*)+34'
  - C: '#11 0x5c56ff4ded3b in fiber_loop+219'
  - C: '#12 0x5c56ff90c11c in coro_init+120'
  csw: 42
  max_slice:
    err: 1
    warn: 0.5
  memory:
    total: 524760
    used: 520
  time: 0
  name: TX_LOOP
  fid: 117
<...snipped...>
```

**Cancel the TX_LOOP fiber by its ID and rollback transaction:**
```
>>> print(wlua.call("pcall(require('fiber').cancel, 117)"))
true
>>> txn.rollback()
2. !!!
```

**Exiting the interactive Python console:**
```
>>> exit()
2025-05-30 12:09:06.143 [48252] main main.cc:1074 I> entering the event loop
```

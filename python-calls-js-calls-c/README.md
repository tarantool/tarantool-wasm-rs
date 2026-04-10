2026-04-10 21:31:19.618 [1907027] unknown say.rs:25 I> python consumer: call-chain-add(20, 22)
2026-04-10 21:31:19.622 [1907027] unknown say.rs:25 I> js provider: add(20, 22)
2026-04-10 21:31:19.622 [1907027] unknown say.rs:25 I> c provider: add-base(20, 22)
2026-04-10 21:31:19.627 [1907027] unknown say.rs:25 I> python consumer: call-chain-label(20, 22)
2026-04-10 21:31:19.628 [1907027] unknown say.rs:25 I> js provider: add(20, 22)
2026-04-10 21:31:19.628 [1907027] unknown say.rs:25 I> c provider: add-base(20, 22)
2026-04-10 21:31:19.628 [1907027] unknown say.rs:25 I> js provider: label(20, 22)
2026-04-10 21:31:19.628 [1907027] unknown say.rs:25 I> c provider: label-base(20, 22)
call-chain-add =>       42
call-chain-label =>     python saw [js wrapped [c says: 20 + 22 = 42]], total = 42
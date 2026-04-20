# smallvec — ETNA Tasks

Total tasks: 12

ETNA tasks are **mutation/property/witness triplets**. Each row below is one runnable task.

## Task Index

| Task | Variant | Framework | Property | Witness | Command |
|------|---------|-----------|----------|---------|---------|
| 001  | `leak_inline_panic_3395246_1`       | proptest    | `property_leak_inline_panics`      | `witness_leak_inline_panics_case_inline_single_byte`                   | `cargo run --release --features etna --bin etna -- proptest LeakInlinePanics` |
| 002  | `leak_inline_panic_3395246_1`       | quickcheck  | `property_leak_inline_panics`      | `witness_leak_inline_panics_case_inline_single_byte`                   | `cargo run --release --features etna --bin etna -- quickcheck LeakInlinePanics` |
| 003  | `leak_inline_panic_3395246_1`       | crabcheck   | `property_leak_inline_panics`      | `witness_leak_inline_panics_case_inline_single_byte`                   | `cargo run --release --features etna --bin etna -- crabcheck LeakInlinePanics` |
| 004  | `leak_inline_panic_3395246_1`       | hegel       | `property_leak_inline_panics`      | `witness_leak_inline_panics_case_inline_single_byte`                   | `cargo run --release --features etna --bin etna -- hegel LeakInlinePanics` |
| 005  | `append_set_len_1bd2dbc_1`          | proptest    | `property_append_preserves_length` | `witness_append_preserves_length_case_small_spilled`                   | `cargo run --release --features etna --bin etna -- proptest AppendPreservesLength` |
| 006  | `append_set_len_1bd2dbc_1`          | quickcheck  | `property_append_preserves_length` | `witness_append_preserves_length_case_small_spilled`                   | `cargo run --release --features etna --bin etna -- quickcheck AppendPreservesLength` |
| 007  | `append_set_len_1bd2dbc_1`          | crabcheck   | `property_append_preserves_length` | `witness_append_preserves_length_case_small_spilled`                   | `cargo run --release --features etna --bin etna -- crabcheck AppendPreservesLength` |
| 008  | `append_set_len_1bd2dbc_1`          | hegel       | `property_append_preserves_length` | `witness_append_preserves_length_case_small_spilled`                   | `cargo run --release --features etna --bin etna -- hegel AppendPreservesLength` |
| 009  | `from_vec_zero_capacity_944f603_1`  | proptest    | `property_from_vec_zero_capacity`  | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip`   | `cargo run --release --features etna --bin etna -- proptest FromVecZeroCapacity` |
| 010  | `from_vec_zero_capacity_944f603_1`  | quickcheck  | `property_from_vec_zero_capacity`  | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip`   | `cargo run --release --features etna --bin etna -- quickcheck FromVecZeroCapacity` |
| 011  | `from_vec_zero_capacity_944f603_1`  | crabcheck   | `property_from_vec_zero_capacity`  | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip`   | `cargo run --release --features etna --bin etna -- crabcheck FromVecZeroCapacity` |
| 012  | `from_vec_zero_capacity_944f603_1`  | hegel       | `property_from_vec_zero_capacity`  | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip`   | `cargo run --release --features etna --bin etna -- hegel FromVecZeroCapacity` |

## Witness catalog

Each witness is a deterministic concrete test. Base build: passes. Variant-active build: fails.

- `witness_leak_inline_panics_case_inline_single_byte` — builds a `SmallVec<u8, 4>` inline via `new()` + `push(0x42)` (cap_tag=1 ⇒ N=4, one byte fits inline) and runs `.leak()` inside `catch_unwind`. Base: `leak` panics via the `spilled()` guard, so `caught.is_err()`. Variant: guard removed, `leak` returns `Ok(&mut [0x42])`, so the property fails with `leak on inline SmallVec<u8, 4> with len=1 did not panic`.
- `witness_append_preserves_length_case_small_spilled` — `a=[1, 2, 3], b=[4, 5]`; `SmallVec::<i64, 4>::from_vec` spills both, so the append hits the spilled branch. Base: post-`append`, `va.len()==5` and `va.iter()` yields `[1,2,3,4,5]`. Variant: `set_len` is removed, so `va.len()` stays at 3 and the property fails with `append len mismatch: got 3, expected 5`.
- `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` — `push_after=[7, 8, 9]`; the property first builds `SmallVec::<u16, 2>::from_vec(Vec::with_capacity(0))` and checks `!sv.spilled()`. Base: the early-return guard makes it inline. Variant: the guard is removed, `from_vec` forwards the dangling pointer into the heap-tagged constructor, so `sv.spilled()==true` and the property fails with `SmallVec::from_vec(Vec::with_capacity(0)) was spilled`.

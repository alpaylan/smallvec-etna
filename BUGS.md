# smallvec — Injected Bugs

ETNA workload for the Rust `smallvec` crate. Each variant re-introduces one
historical bug-fix into a fresh patched branch and pairs it with a
framework-neutral property, four PBT adapters, and a deterministic witness
test.

Total mutations: 3

## Bug Index

| # | Variant | Name | Location | Injection | Fix Commit |
|---|---------|------|----------|-----------|------------|
| 1 | `append_set_len_1bd2dbc_1` | `append_set_len` | `src/lib.rs` | `patch` | `1bd2dbce81d9b58214bbbf5de30323017705d5a8` |
| 2 | `from_vec_zero_capacity_944f603_1` | `from_vec_zero_capacity` | `src/lib.rs` | `patch` | `944f60397a35043ef5704e9bf76b26cdcb5fc55d` |
| 3 | `leak_inline_panic_3395246_1` | `leak_inline_panic` | `src/lib.rs` | `patch` | `339524684344f74ea7e5e05b3b5ae4e5043999f8` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `append_set_len_1bd2dbc_1` | `AppendPreservesLength` | `witness_append_preserves_length_case_small_spilled` |
| `from_vec_zero_capacity_944f603_1` | `FromVecZeroCapacity` | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` |
| `leak_inline_panic_3395246_1` | `LeakInlinePanics` | `witness_leak_inline_panics_case_inline_single_byte` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `AppendPreservesLength` | ✓ | ✓ | ✓ | ✓ |
| `FromVecZeroCapacity` | ✓ | ✓ | ✓ | ✓ |
| `LeakInlinePanics` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. append_set_len

- **Variant**: `append_set_len_1bd2dbc_1`
- **Location**: `src/lib.rs`
- **Property**: `AppendPreservesLength`
- **Witness(es)**:
  - `witness_append_preserves_length_case_small_spilled`
- **Source**: bugfix for append not set self.len properly (#347)
  > `SmallVec::append` copied bytes into the destination buffer but forgot to call `set_len`, so after the call the logical length still reflected the pre-append state and the appended elements were invisible.
- **Fix commit**: `1bd2dbce81d9b58214bbbf5de30323017705d5a8` — bugfix for append not set self.len properly (#347)
- **Invariant violated**: After `a.append(&mut b)`, `a.len()` equals `a.len() + b.len()` (pre-call) and every element is visible via iteration.
- **How the mutation triggers**: Removes the trailing `unsafe { self.set_len(total_len) }` call in `SmallVec::append`, so the copy into the destination buffer happens but the logical length is never bumped. The property computes the expected concatenation length, compares against `va.len()`, and also checks that `va.iter().copied().collect::<Vec<_>>()` matches the concat — all three fail because `va.len()` stays at its pre-append value.

### 2. from_vec_zero_capacity

- **Variant**: `from_vec_zero_capacity_944f603_1`
- **Location**: `src/lib.rs`
- **Property**: `FromVecZeroCapacity`
- **Witness(es)**:
  - `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip`
- **Source**: Correctly handle Vec with zero capacity in from_vec (#332)
  > `SmallVec::from_vec` wrapped a zero-capacity `Vec`'s dangling pointer directly into `RawSmallVec::new_heap` and tagged the result as spilled; the fix short-circuits to an empty inline SmallVec to avoid later UB in shrink/deallocate/push.
- **Fix commit**: `944f60397a35043ef5704e9bf76b26cdcb5fc55d` — Correctly handle Vec with zero capacity in from_vec (#332)
- **Invariant violated**: `SmallVec::from_vec(Vec::with_capacity(0))` must be inline (non-spilled). A zero-capacity `Vec` holds a dangling but non-null pointer; wrapping that pointer into a heap-tagged SmallVec produces an object that later UBs under `shrink_to_fit`, `deallocate`, or `push`.
- **How the mutation triggers**: Removes the `if vec.capacity() == 0 { return Self::new(); }` early return, so `from_vec` forwards a dangling pointer into `RawSmallVec::new_heap` and tags the result as spilled. The property observes `sv.spilled() == true` on a SmallVec built from `Vec::with_capacity(0)` and fails immediately (without having to trigger the follow-on UB).

### 3. leak_inline_panic

- **Variant**: `leak_inline_panic_3395246_1`
- **Location**: `src/lib.rs`
- **Property**: `LeakInlinePanics`
- **Witness(es)**:
  - `witness_leak_inline_panics_case_inline_single_byte`
- **Source**: Fix: SmallVec::leak() to panic on inline storage (GHSA-5h7v-3586-wm8c)
  > `SmallVec::leak` handed back a `&mut [T]` borrowed from the consumed value's stack storage when the vector was still inline (GHSA-5h7v-3586-wm8c); the fix panics rather than returning a dangling reference.
- **Fix commit**: `339524684344f74ea7e5e05b3b5ae4e5043999f8` — Fix: SmallVec::leak() to panic on inline storage (GHSA-5h7v-3586-wm8c)
- **Invariant violated**: `SmallVec::<T, N>::leak(self)` on an inline (not spilled) vector must panic. Returning a slice that borrows into the consumed stack frame would be unsound — any caller would receive a reference to freed storage.
- **How the mutation triggers**: Removes the `if !self.spilled() { panic!(...) }` guard inside `leak`, so inline SmallVecs silently hand out a `&mut [T]` into the about-to-be-dropped stack buffer. The property builds an inline `SmallVec<u8, N>` via `SmallVec::new()` + `push`, runs `leak()` under `catch_unwind`, and reports `Fail` if the call returns `Ok`.

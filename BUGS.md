# smallvec — Injected Bugs

Total mutations: 3

## Bug Index

| # | Name | Variant | File | Injection | Fix Commit |
|---|------|---------|------|-----------|------------|
| 1 | `SmallVec::leak on inline panics` | `leak_inline_panic_3395246_1` | `patches/leak_inline_panic_3395246_1.patch` | `patch` | `339524684344f74ea7e5e05b3b5ae4e5043999f8` |
| 2 | `SmallVec::append updates self.len` | `append_set_len_1bd2dbc_1` | `patches/append_set_len_1bd2dbc_1.patch` | `patch` | `1bd2dbce81d9b58214bbbf5de30323017705d5a8` |
| 3 | `SmallVec::from_vec handles zero-capacity Vec` | `from_vec_zero_capacity_944f603_1` | `patches/from_vec_zero_capacity_944f603_1.patch` | `patch` | `944f60397a35043ef5704e9bf76b26cdcb5fc55d` |

## Property Mapping

| Variant | Property | Witness(es) |
|---------|----------|-------------|
| `leak_inline_panic_3395246_1` | `property_leak_inline_panics` | `witness_leak_inline_panics_case_inline_single_byte` |
| `append_set_len_1bd2dbc_1` | `property_append_preserves_length` | `witness_append_preserves_length_case_small_spilled` |
| `from_vec_zero_capacity_944f603_1` | `property_from_vec_zero_capacity` | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` |

## Framework Coverage

| Property | proptest | quickcheck | crabcheck | hegel |
|----------|---------:|-----------:|----------:|------:|
| `property_leak_inline_panics` | ✓ | ✓ | ✓ | ✓ |
| `property_append_preserves_length` | ✓ | ✓ | ✓ | ✓ |
| `property_from_vec_zero_capacity` | ✓ | ✓ | ✓ | ✓ |

## Bug Details

### 1. SmallVec::leak on inline panics

- **Variant**: `leak_inline_panic_3395246_1`
- **Location**: `patches/leak_inline_panic_3395246_1.patch`
- **Property**: `property_leak_inline_panics`
- **Witness(es)**: `witness_leak_inline_panics_case_inline_single_byte`
- **Fix commit**: `339524684344f74ea7e5e05b3b5ae4e5043999f8` — `Fix: SmallVec::leak() to panic on inline storage (GHSA-5h7v-3586-wm8c)`
- **Invariant violated**: `SmallVec::<T, N>::leak(self)` on an inline (not spilled) vector must panic. Returning a slice that borrows into the consumed stack frame would be unsound — any caller would receive a reference to freed storage.
- **How the mutation triggers**: Removes the `if !self.spilled() { panic!(...) }` guard inside `leak`, so inline SmallVecs silently hand out a `&mut [T]` into the about-to-be-dropped stack buffer. The property builds an inline `SmallVec<u8, N>` via `SmallVec::new()` + `push`, runs `leak()` under `catch_unwind`, and reports `Fail` if the call returns `Ok`.

### 2. SmallVec::append updates self.len

- **Variant**: `append_set_len_1bd2dbc_1`
- **Location**: `patches/append_set_len_1bd2dbc_1.patch`
- **Property**: `property_append_preserves_length`
- **Witness(es)**: `witness_append_preserves_length_case_small_spilled`
- **Fix commit**: `1bd2dbce81d9b58214bbbf5de30323017705d5a8` — `bugfix for append not set self.len properly (#347)`
- **Invariant violated**: After `a.append(&mut b)`, `a.len()` equals `a.len() + b.len()` (pre-call) and every element is visible via iteration.
- **How the mutation triggers**: Removes the trailing `unsafe { self.set_len(total_len) }` call in `SmallVec::append`, so the copy into the destination buffer happens but the logical length is never bumped. The property computes the expected concatenation length, compares against `va.len()`, and also checks that `va.iter().copied().collect::<Vec<_>>()` matches the concat — all three fail because `va.len()` stays at its pre-append value.

### 3. SmallVec::from_vec handles zero-capacity Vec

- **Variant**: `from_vec_zero_capacity_944f603_1`
- **Location**: `patches/from_vec_zero_capacity_944f603_1.patch`
- **Property**: `property_from_vec_zero_capacity`
- **Witness(es)**: `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip`
- **Fix commit**: `944f60397a35043ef5704e9bf76b26cdcb5fc55d` — `Correctly handle Vec with zero capacity in from_vec (#332)`
- **Invariant violated**: `SmallVec::from_vec(Vec::with_capacity(0))` must be inline (non-spilled). A zero-capacity `Vec` holds a dangling but non-null pointer; wrapping that pointer into a heap-tagged SmallVec produces an object that later UBs under `shrink_to_fit`, `deallocate`, or `push`.
- **How the mutation triggers**: Removes the `if vec.capacity() == 0 { return Self::new(); }` early return, so `from_vec` forwards a dangling pointer into `RawSmallVec::new_heap` and tags the result as spilled. The property observes `sv.spilled() == true` on a SmallVec built from `Vec::with_capacity(0)` and fails immediately (without having to trigger the follow-on UB).

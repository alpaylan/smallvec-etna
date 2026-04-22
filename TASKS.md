# smallvec — ETNA Tasks

Total tasks: 12

## Task Index

| Task | Variant | Framework | Property | Witness |
|------|---------|-----------|----------|---------|
| 001 | `append_set_len_1bd2dbc_1` | proptest | `AppendPreservesLength` | `witness_append_preserves_length_case_small_spilled` |
| 002 | `append_set_len_1bd2dbc_1` | quickcheck | `AppendPreservesLength` | `witness_append_preserves_length_case_small_spilled` |
| 003 | `append_set_len_1bd2dbc_1` | crabcheck | `AppendPreservesLength` | `witness_append_preserves_length_case_small_spilled` |
| 004 | `append_set_len_1bd2dbc_1` | hegel | `AppendPreservesLength` | `witness_append_preserves_length_case_small_spilled` |
| 005 | `from_vec_zero_capacity_944f603_1` | proptest | `FromVecZeroCapacity` | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` |
| 006 | `from_vec_zero_capacity_944f603_1` | quickcheck | `FromVecZeroCapacity` | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` |
| 007 | `from_vec_zero_capacity_944f603_1` | crabcheck | `FromVecZeroCapacity` | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` |
| 008 | `from_vec_zero_capacity_944f603_1` | hegel | `FromVecZeroCapacity` | `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` |
| 009 | `leak_inline_panic_3395246_1` | proptest | `LeakInlinePanics` | `witness_leak_inline_panics_case_inline_single_byte` |
| 010 | `leak_inline_panic_3395246_1` | quickcheck | `LeakInlinePanics` | `witness_leak_inline_panics_case_inline_single_byte` |
| 011 | `leak_inline_panic_3395246_1` | crabcheck | `LeakInlinePanics` | `witness_leak_inline_panics_case_inline_single_byte` |
| 012 | `leak_inline_panic_3395246_1` | hegel | `LeakInlinePanics` | `witness_leak_inline_panics_case_inline_single_byte` |

## Witness Catalog

- `witness_append_preserves_length_case_small_spilled` — base passes, variant fails
- `witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip` — base passes, variant fails
- `witness_leak_inline_panics_case_inline_single_byte` — base passes, variant fails

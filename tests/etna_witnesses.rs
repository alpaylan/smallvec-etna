//! ETNA witness tests — each `witness_<name>_case_<tag>` calls one property
//! function with the exact canonical input embedded in the `etna` replay
//! path (src/bin/etna.rs). On base, every witness passes. On its paired
//! variant branch (patched with the bug), the corresponding witness fails.
//!
//! Keep the inputs here in sync with `canonical_*` in src/bin/etna.rs.

#![cfg(feature = "etna")]

use smallvec::etna::{
    property_append_preserves_length, property_from_vec_zero_capacity, property_leak_inline_panics,
    PropertyResult,
};

fn assert_pass(r: PropertyResult) {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => {}
        PropertyResult::Fail(m) => panic!("property failed: {}", m),
    }
}

/// Triggers `leak_inline_panic_3395246_1`. A single-element `SmallVec<u8, 4>`
/// is inline (not spilled), so the buggy `leak` will return a slice into a
/// consumed stack frame rather than panicking. The property observes this by
/// running `leak` inside `catch_unwind`.
#[test]
fn witness_leak_inline_panics_case_inline_single_byte() {
    // inline_cap % 3 == 1 picks N = 4; 1 byte fits inline.
    assert_pass(property_leak_inline_panics(vec![0x42], 1));
}

/// Triggers `append_set_len_1bd2dbc_1`. The concatenation of `[1, 2, 3]` and
/// `[4, 5]` is length 5, but the buggy `append` leaves `self.len()` at 3.
#[test]
fn witness_append_preserves_length_case_small_spilled() {
    assert_pass(property_append_preserves_length(vec![1, 2, 3], vec![4, 5]));
}

/// Triggers `from_vec_zero_capacity_944f603_1`. Starting from a
/// `Vec::with_capacity(0)`, the buggy `from_vec` constructs a SmallVec
/// tagged as "spilled" with a dangling heap pointer. The property observes
/// the bogus `spilled()` flag directly.
#[test]
fn witness_from_vec_zero_capacity_case_empty_capacity_push_round_trip() {
    assert_pass(property_from_vec_zero_capacity(vec![7, 8, 9]));
}

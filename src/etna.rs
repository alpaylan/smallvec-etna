//! ETNA framework-neutral property functions for the smallvec crate.
//!
//! Each `property_<name>` is a pure function taking concrete, owned inputs
//! and returning `PropertyResult`. Framework adapters in `src/bin/etna.rs`
//! and witness tests in `tests/etna_witnesses.rs` all call these functions
//! directly.

#![allow(missing_docs)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::SmallVec;

#[derive(Debug)]
pub enum PropertyResult {
    Pass,
    Fail(String),
    Discard,
}

/// Invariant: `SmallVec::<T, N>::leak(v)` on an inline (not spilled) vector
/// must panic rather than return a dangling slice. Returning a slice into a
/// consumed stack frame is unsound; the historical fix added a `spilled()`
/// check that panics on the inline path.
///
/// Bug this catches:
/// - `leak_inline_panic_3395246_1`: the pre-fix `leak` unconditionally forged
///   a `&'a mut [T]` from an inline buffer. The mutation removes the `spilled`
///   guard and restores that behavior. The property invokes `leak` inside
///   `catch_unwind`; if it returns Ok, the guard is missing and the property
///   fails.
pub fn property_leak_inline_panics(values: Vec<u8>, inline_cap: u8) -> PropertyResult {
    // `inline_cap` has to match a concrete const N in SmallVec's type parameter.
    // We support N in {1, 4, 16} which covers inline-small, inline-medium and
    // always-spilled (when values.len() > 16).
    let cap = match inline_cap % 3 {
        0 => 1u8,
        1 => 4u8,
        _ => 16u8,
    };

    match cap {
        1 => property_leak_inline_panics_n::<1>(values),
        4 => property_leak_inline_panics_n::<4>(values),
        _ => property_leak_inline_panics_n::<16>(values),
    }
}

fn property_leak_inline_panics_n<const N: usize>(values: Vec<u8>) -> PropertyResult {
    let n = values.len();
    // Build the SmallVec inline when it fits: `from_vec` in smallvec v2 wraps the
    // heap allocation of the source `Vec`, so it always comes back spilled for
    // non-empty input. Use `new()` + `push()` so short inputs stay inline and
    // exercise the bug the property is meant to catch.
    let v: SmallVec<u8, N> = if n <= N {
        let mut sv: SmallVec<u8, N> = SmallVec::new();
        for &x in &values {
            sv.push(x);
        }
        sv
    } else {
        SmallVec::from_vec(values.clone())
    };
    let spilled = v.spilled();

    // On inline storage, `leak` must panic. On spilled storage, it must succeed
    // and the returned slice must contain the original bytes.
    if !spilled {
        let caught = std::panic::catch_unwind(move || {
            let _slice: &mut [u8] = v.leak();
        });
        if caught.is_ok() {
            return PropertyResult::Fail(format!(
                "leak on inline SmallVec<u8, {}> with len={} did not panic",
                N, n
            ));
        }
        PropertyResult::Pass
    } else {
        let caught = std::panic::catch_unwind(move || {
            // We deliberately forget the returned slice so no double-free
            // occurs on the heap allocation; the point is that it must not
            // panic on the spilled path.
            let slice: &mut [u8] = v.leak();
            // Compare values against the original input.
            let copy: Vec<u8> = slice.to_vec();
            // Deallocate the leaked heap buffer so Miri/valgrind don't flag it.
            let raw = slice.as_mut_ptr();
            let len = slice.len();
            unsafe {
                let _ = Vec::from_raw_parts(raw, len, len);
            }
            copy
        });
        match caught {
            Ok(copy) => {
                if copy == values {
                    PropertyResult::Pass
                } else {
                    PropertyResult::Fail(format!(
                        "leak on spilled SmallVec<u8, {}> returned wrong bytes: {:?} vs input {:?}",
                        N, copy, values
                    ))
                }
            }
            Err(_) => PropertyResult::Fail(format!(
                "leak on spilled SmallVec<u8, {}> with len={} panicked",
                N, n
            )),
        }
    }
}

/// Invariant: after `v.append(&mut other)`, `v.len()` equals the sum of the
/// two pre-call lengths, and `other.len()` is zero. In particular, `v.len()`
/// must reflect the new logical length so that subsequent operations like
/// `iter().collect::<Vec<_>>()` see every element.
///
/// Bug this catches:
/// - `append_set_len_1bd2dbc_1`: the pre-fix `append` moved bytes into the
///   destination buffer but never called `self.set_len(total_len)`. The
///   mutation removes that `set_len` call, so `v.len()` stays equal to its
///   pre-append value and `v.iter().count()` under-counts by `other.len()`.
pub fn property_append_preserves_length(a: Vec<i64>, b: Vec<i64>) -> PropertyResult {
    let mut va: SmallVec<i64, 4> = SmallVec::from_vec(a.clone());
    let mut vb: SmallVec<i64, 4> = SmallVec::from_vec(b.clone());

    let expected_len = a.len() + b.len();
    va.append(&mut vb);

    if va.len() != expected_len {
        return PropertyResult::Fail(format!(
            "append len mismatch: got {}, expected {}",
            va.len(),
            expected_len
        ));
    }
    if vb.len() != 0 {
        return PropertyResult::Fail(format!(
            "append did not drain other: other.len()={}",
            vb.len()
        ));
    }

    // Check that every element is visible via iteration (catches a stale len).
    let collected: Vec<i64> = va.iter().copied().collect();
    if collected.len() != expected_len {
        return PropertyResult::Fail(format!(
            "append iter count {} != expected {}",
            collected.len(),
            expected_len
        ));
    }
    let mut concat = a.clone();
    concat.extend(b.iter().copied());
    if collected != concat {
        return PropertyResult::Fail(format!(
            "append content mismatch: got {:?}, expected {:?}",
            collected, concat
        ));
    }
    PropertyResult::Pass
}

/// Invariant: `SmallVec::from_vec(v)` must always return a well-formed
/// SmallVec regardless of the source `Vec`'s capacity. In particular, when
/// the `Vec` has capacity zero (and therefore a dangling allocation pointer),
/// the resulting SmallVec must still behave correctly under subsequent
/// operations such as `shrink_to_fit`, `push`, and `iter`.
///
/// Bug this catches:
/// - `from_vec_zero_capacity_944f603_1`: the pre-fix `from_vec` wrapped the
///   dangling pointer of a zero-capacity `Vec` into `RawSmallVec::new_heap`
///   and tagged the SmallVec as "spilled". Later calls to `shrink_to_fit`,
///   `deallocate`, or `push` then dereferenced or freed the dangling pointer.
///   The mutation removes the early `if vec.capacity() == 0 { return ... }`
///   guard. The property triggers by building from an empty-capacity `Vec`
///   and then pushing / shrinking; a crash or observable corruption under
///   `shrink_to_fit` + iteration fails the property.
pub fn property_from_vec_zero_capacity(push_after: Vec<u16>) -> PropertyResult {
    let src: Vec<u16> = Vec::with_capacity(0);
    let mut sv: SmallVec<u16, 2> = SmallVec::from_vec(src);

    // After construction from an empty zero-capacity Vec, the SmallVec must
    // be inline (non-spilled): the bug that the fix guards against is the
    // "tagged spilled with a dangling pointer" state.
    if sv.spilled() {
        return PropertyResult::Fail(format!(
            "SmallVec::from_vec(Vec::with_capacity(0)) was spilled (len={}, cap={})",
            sv.len(),
            sv.capacity()
        ));
    }
    if sv.len() != 0 {
        return PropertyResult::Fail(format!(
            "SmallVec::from_vec(Vec::with_capacity(0)).len()={}",
            sv.len()
        ));
    }

    // Exercise shrink_to_fit — the historical bug crashed here because the
    // allocator saw a dangling pointer with a non-zero capacity tag.
    sv.shrink_to_fit();
    if sv.spilled() {
        return PropertyResult::Fail(String::from(
            "shrink_to_fit left an empty SmallVec spilled",
        ));
    }

    // Push the full input and confirm round-trip.
    for &x in &push_after {
        sv.push(x);
    }
    if sv.len() != push_after.len() {
        return PropertyResult::Fail(format!(
            "after pushes: len={}, expected {}",
            sv.len(),
            push_after.len()
        ));
    }
    let collected: Vec<u16> = sv.iter().copied().collect();
    if collected != push_after {
        return PropertyResult::Fail(format!(
            "content mismatch: got {:?}, expected {:?}",
            collected, push_after
        ));
    }
    PropertyResult::Pass
}

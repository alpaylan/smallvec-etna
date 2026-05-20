//! Fault-localization integration tests for smallvec.

#![cfg(feature = "etna")]

use std::fmt;

use crabcheck::quickcheck::{Arbitrary, Mutate};
use rand_etna::Rng;
use smallvec::etna::{
    property_append_preserves_length, property_from_vec_zero_capacity,
    property_leak_inline_panics, PropertyResult,
};

#[derive(Clone)]
struct LeakInput { values: Vec<u8>, inline_cap: u8 }
impl fmt::Debug for LeakInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "values={:?} cap_tag={}", self.values, self.inline_cap)
    }
}

#[derive(Clone)]
struct AppendInput { a: Vec<i64>, b: Vec<i64> }
impl fmt::Debug for AppendInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a={:?} b={:?}", self.a, self.b)
    }
}

#[derive(Clone)]
struct FromVecZeroInput { data: Vec<u16>, reserve: u8 }
impl fmt::Debug for FromVecZeroInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "data={:?} reserve={}", self.data, self.reserve)
    }
}

fn gen_small_vec_u8<R: Rng>(rng: &mut R) -> Vec<u8> {
    let n: usize = (rng.random::<u32>() as usize) % 32;
    (0..n).map(|_| rng.random::<u8>()).collect()
}
fn gen_small_vec_i64<R: Rng>(rng: &mut R) -> Vec<i64> {
    let n: usize = (rng.random::<u32>() as usize) % 16;
    (0..n).map(|_| rng.random::<i64>()).collect()
}
fn gen_small_vec_u16<R: Rng>(rng: &mut R) -> Vec<u16> {
    // Short Vec range so capacity-0 inputs (the FromVecZeroCapacity bug
    // trigger) appear with non-trivial frequency.
    let n: usize = (rng.random::<u32>() as usize) % 8;
    (0..n).map(|_| rng.random::<u16>()).collect()
}

impl<R: Rng> Arbitrary<R> for LeakInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        LeakInput { values: gen_small_vec_u8(rng), inline_cap: rng.random() }
    }
}
impl<R: Rng> Arbitrary<R> for AppendInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        AppendInput { a: gen_small_vec_i64(rng), b: gen_small_vec_i64(rng) }
    }
}
impl<R: Rng> Arbitrary<R> for FromVecZeroInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        FromVecZeroInput {
            data: gen_small_vec_u16(rng),
            reserve: rng.random::<u8>() % 4,
        }
    }
}

impl<R: Rng> Mutate<R> for LeakInput {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut out = self.clone();
        match rng.random_range(0u8..3) {
            0 => { let b = rng.random_range(0u32..8); out.inline_cap ^= 1u8 << b; },
            1 if !out.values.is_empty() => {
                let i = rng.random_range(0..out.values.len());
                let b = rng.random_range(0u32..8);
                out.values[i] ^= 1u8 << b;
            },
            _ => {
                if rng.random_bool(0.5) && out.values.len() < 32 {
                    out.values.push(rng.random());
                } else if !out.values.is_empty() { out.values.pop(); }
            },
        }
        out
    }
}

fn mutate_vec_i64<R: Rng>(v: &[i64], rng: &mut R) -> Vec<i64> {
    let mut out = v.to_vec();
    match rng.random_range(0u8..3) {
        0 if !out.is_empty() => {
            let i = rng.random_range(0..out.len());
            let delta: i64 = (rng.random_range(1..=10i64))
                * if rng.random_bool(0.5) { 1 } else { -1 };
            out[i] = out[i].saturating_add(delta);
        },
        1 if out.len() < 16 => out.push(rng.random::<i64>()),
        _ if !out.is_empty() => { out.pop(); },
        _ => {},
    }
    out
}

impl<R: Rng> Mutate<R> for AppendInput {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut out = self.clone();
        if rng.random_bool(0.5) { out.a = mutate_vec_i64(&out.a, rng); }
        else { out.b = mutate_vec_i64(&out.b, rng); }
        out
    }
}

impl<R: Rng> Mutate<R> for FromVecZeroInput {
    fn mutate(&self, rng: &mut R, _n: usize) -> Self {
        let mut out = self.clone();
        match rng.random_range(0u8..4) {
            0 if !out.data.is_empty() => {
                let i = rng.random_range(0..out.data.len());
                let b = rng.random_range(0u32..16);
                out.data[i] ^= 1u16 << b;
            },
            1 if out.data.len() < 8 => out.data.push(rng.random()),
            2 if !out.data.is_empty() => { out.data.pop(); },
            _ => { out.reserve = rng.random::<u8>() % 4; },
        }
        out
    }
}

fn to_opt(r: PropertyResult) -> Option<bool> {
    match r {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn property_leak_inline_panics_test(i: LeakInput) -> Option<bool> {
    to_opt(property_leak_inline_panics(i.values, i.inline_cap))
}
fn property_append_preserves_length_test(i: AppendInput) -> Option<bool> {
    to_opt(property_append_preserves_length(i.a, i.b))
}
fn property_from_vec_zero_capacity_test(i: FromVecZeroInput) -> Option<bool> {
    to_opt(property_from_vec_zero_capacity(i.data, i.reserve))
}

fn emit_locate_json(r: &crabcheck::profiling::LocateResult) {
    use crabcheck::quickcheck::ResultStatus;
    let status = match &r.run.status {
        ResultStatus::Failed { .. } => "Failed",
        ResultStatus::Finished => "Finished",
        ResultStatus::GaveUp => "GaveUp",
        ResultStatus::TimedOut => "TimedOut",
        ResultStatus::Aborted { .. } => "Aborted",
    };
    let top = if let Some(s) = r.top() {
        serde_json::json!({
            "rank": s.rank, "file": s.region.file, "function": s.region.function,
            "start_line": s.region.start_line, "end_line": s.region.end_line,
            "ochiai": s.region.suspiciousness.ochiai, "delta": s.region.delta,
            "panic_overlap": s.panic_overlap,
            "confidence": format!("{}", s.confidence),
            "confidence_rule": s.confidence_rule,
        })
    } else { serde_json::Value::Null };
    let top_5: Vec<_> = r.suspects.iter().take(5).map(|s| serde_json::json!({
        "rank": s.rank, "file": s.region.file, "function": s.region.function,
        "start_line": s.region.start_line, "end_line": s.region.end_line,
        "confidence": format!("{}", s.confidence),
        "confidence_rule": s.confidence_rule,
        "panic_overlap": s.panic_overlap,
    })).collect();
    let diags: Vec<_> = r.diagnostics.iter().map(|d| d.tag()).collect();
    let out = serde_json::json!({
        "status": status, "passed": r.run.passed, "discarded": r.run.discarded,
        "n_panics": r.n_panics, "n_suspects": r.suspects.len(),
        "top": top, "top_5": top_5, "diagnostics": diags,
    });
    println!("@@LOCATE@@ {}", out);
}

#[test]
fn locate_leak_inline_panics() {
    let report = crabcheck::quickcheck_with_locate!(property_leak_inline_panics_test, "smallvec");
    eprintln!("{report}");
    emit_locate_json(&report);
}

#[test]
fn locate_append_preserves_length() {
    let report = crabcheck::quickcheck_with_locate!(property_append_preserves_length_test, "smallvec");
    eprintln!("{report}");
    emit_locate_json(&report);
}

#[test]
fn locate_from_vec_zero_capacity() {
    let report = crabcheck::quickcheck_with_locate!(property_from_vec_zero_capacity_test, "smallvec");
    eprintln!("{report}");
    emit_locate_json(&report);
}

// ETNA workload runner for smallvec.
//
// Usage: cargo run --release --bin etna -- <tool> <property>
//   tool:     etna | proptest | quickcheck | crabcheck | hegel
//   property: LeakInlinePanics | AppendPreservesLength | FromVecZeroCapacity | All
//
// Every invocation prints exactly one JSON line to stdout and exits 0
// (except argv parsing, which exits 2).

use crabcheck::quickcheck as crabcheck_qc;
use crabcheck::quickcheck::Arbitrary as CcArbitrary;
use hegel::{generators as hgen, HealthCheck, Hegel, Settings as HegelSettings, TestCase};
use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestError};
use quickcheck_etna::{Arbitrary as QcArbitrary, Gen, QuickCheck, ResultStatus, TestResult};
use rand_etna::Rng;
use smallvec::etna::{
    property_append_preserves_length, property_from_vec_zero_capacity,
    property_leak_inline_panics, PropertyResult,
};

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Default, Clone, Copy)]
struct Metrics {
    inputs: u64,
    elapsed_us: u128,
}

impl Metrics {
    fn combine(self, other: Metrics) -> Metrics {
        Metrics {
            inputs: self.inputs + other.inputs,
            elapsed_us: self.elapsed_us + other.elapsed_us,
        }
    }
}

type Outcome = (Result<(), String>, Metrics);

fn to_err(r: PropertyResult) -> Result<(), String> {
    match r {
        PropertyResult::Pass | PropertyResult::Discard => Ok(()),
        PropertyResult::Fail(m) => Err(m),
    }
}

const ALL_PROPERTIES: &[&str] = &[
    "LeakInlinePanics",
    "AppendPreservesLength",
    "FromVecZeroCapacity",
];

fn cases_budget() -> u64 {
    std::env::var("ETNA_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200)
}

fn run_all<F: FnMut(&str) -> Outcome>(mut f: F) -> Outcome {
    let mut total = Metrics::default();
    for p in ALL_PROPERTIES {
        let (r, m) = f(p);
        total = total.combine(m);
        if let Err(e) = r {
            return (Err(e), total);
        }
    }
    (Ok(()), total)
}

// ============================================================================
// Input wrappers — each property's concrete args bundled into one value so
// quickcheck / crabcheck Arbitrary impls can describe them as a single param.
// ============================================================================

#[derive(Clone)]
struct LeakInput {
    values: Vec<u8>,
    inline_cap: u8,
}

impl fmt::Debug for LeakInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "values={:?} cap_tag={}", self.values, self.inline_cap)
    }
}

impl fmt::Display for LeakInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Clone)]
struct AppendInput {
    a: Vec<i64>,
    b: Vec<i64>,
}

impl fmt::Debug for AppendInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a={:?} b={:?}", self.a, self.b)
    }
}

impl fmt::Display for AppendInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Clone)]
struct FromVecZeroInput {
    push_after: Vec<u16>,
}

impl fmt::Debug for FromVecZeroInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "push_after={:?}", self.push_after)
    }
}

impl fmt::Display for FromVecZeroInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// ============================================================================
// Canonical witness inputs — used by `tool=etna`. Must match witness tests
// in tests/etna_witnesses.rs.
// ============================================================================

fn canonical_leak() -> LeakInput {
    LeakInput {
        values: vec![0x42],
        inline_cap: 1,
    }
}

fn canonical_append() -> AppendInput {
    AppendInput {
        a: vec![1, 2, 3],
        b: vec![4, 5],
    }
}

fn canonical_from_vec_zero() -> FromVecZeroInput {
    FromVecZeroInput {
        push_after: vec![7, 8, 9],
    }
}

fn check_leak_inline_panics() -> Result<(), String> {
    let v = canonical_leak();
    to_err(property_leak_inline_panics(v.values, v.inline_cap))
}

fn check_append_preserves_length() -> Result<(), String> {
    let v = canonical_append();
    to_err(property_append_preserves_length(v.a, v.b))
}

fn check_from_vec_zero_capacity() -> Result<(), String> {
    let v = canonical_from_vec_zero();
    to_err(property_from_vec_zero_capacity(v.push_after))
}

fn run_etna_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_etna_property);
    }
    let t0 = Instant::now();
    let result = match property {
        "LeakInlinePanics" => check_leak_inline_panics(),
        "AppendPreservesLength" => check_append_preserves_length(),
        "FromVecZeroCapacity" => check_from_vec_zero_capacity(),
        _ => {
            return (
                Err(format!("Unknown property for etna: {property}")),
                Metrics::default(),
            );
        }
    };
    (
        result,
        Metrics {
            inputs: 1,
            elapsed_us: t0.elapsed().as_micros(),
        },
    )
}

// ============================================================================
// quickcheck Arbitrary
// ============================================================================

fn qc_gen_small_vec_u8(g: &mut Gen) -> Vec<u8> {
    // Keep vectors small so property execution is fast; the bugs trigger on
    // len <= 16 in every variant.
    let n: usize = <usize as QcArbitrary>::arbitrary(g) % 32;
    (0..n).map(|_| <u8 as QcArbitrary>::arbitrary(g)).collect()
}

fn qc_gen_small_vec_i64(g: &mut Gen) -> Vec<i64> {
    let n: usize = <usize as QcArbitrary>::arbitrary(g) % 16;
    (0..n).map(|_| <i64 as QcArbitrary>::arbitrary(g)).collect()
}

fn qc_gen_small_vec_u16(g: &mut Gen) -> Vec<u16> {
    let n: usize = <usize as QcArbitrary>::arbitrary(g) % 16;
    (0..n).map(|_| <u16 as QcArbitrary>::arbitrary(g)).collect()
}

impl QcArbitrary for LeakInput {
    fn arbitrary(g: &mut Gen) -> Self {
        LeakInput {
            values: qc_gen_small_vec_u8(g),
            inline_cap: <u8 as QcArbitrary>::arbitrary(g),
        }
    }
}

impl QcArbitrary for AppendInput {
    fn arbitrary(g: &mut Gen) -> Self {
        AppendInput {
            a: qc_gen_small_vec_i64(g),
            b: qc_gen_small_vec_i64(g),
        }
    }
}

impl QcArbitrary for FromVecZeroInput {
    fn arbitrary(g: &mut Gen) -> Self {
        FromVecZeroInput {
            push_after: qc_gen_small_vec_u16(g),
        }
    }
}

// ============================================================================
// crabcheck Arbitrary
// ============================================================================

fn cc_gen_small_vec_u8<R: Rng>(rng: &mut R) -> Vec<u8> {
    let n: usize = (rng.random::<u32>() as usize) % 32;
    (0..n).map(|_| rng.random::<u8>()).collect()
}

fn cc_gen_small_vec_i64<R: Rng>(rng: &mut R) -> Vec<i64> {
    let n: usize = (rng.random::<u32>() as usize) % 16;
    (0..n).map(|_| rng.random::<i64>()).collect()
}

fn cc_gen_small_vec_u16<R: Rng>(rng: &mut R) -> Vec<u16> {
    let n: usize = (rng.random::<u32>() as usize) % 16;
    (0..n).map(|_| rng.random::<u16>()).collect()
}

impl<R: Rng> CcArbitrary<R> for LeakInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        LeakInput {
            values: cc_gen_small_vec_u8(rng),
            inline_cap: rng.random(),
        }
    }
}

impl<R: Rng> CcArbitrary<R> for AppendInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        AppendInput {
            a: cc_gen_small_vec_i64(rng),
            b: cc_gen_small_vec_i64(rng),
        }
    }
}

impl<R: Rng> CcArbitrary<R> for FromVecZeroInput {
    fn generate(rng: &mut R, _n: usize) -> Self {
        FromVecZeroInput {
            push_after: cc_gen_small_vec_u16(rng),
        }
    }
}

// ============================================================================
// proptest strategies
// ============================================================================

fn leak_strategy() -> BoxedStrategy<LeakInput> {
    (proptest::collection::vec(any::<u8>(), 0..32usize), any::<u8>())
        .prop_map(|(values, inline_cap)| LeakInput { values, inline_cap })
        .boxed()
}

fn append_strategy() -> BoxedStrategy<AppendInput> {
    (
        proptest::collection::vec(any::<i64>(), 0..16usize),
        proptest::collection::vec(any::<i64>(), 0..16usize),
    )
        .prop_map(|(a, b)| AppendInput { a, b })
        .boxed()
}

fn from_vec_zero_strategy() -> BoxedStrategy<FromVecZeroInput> {
    proptest::collection::vec(any::<u16>(), 0..16usize)
        .prop_map(|push_after| FromVecZeroInput { push_after })
        .boxed()
}

// ============================================================================
// proptest adapter
// ============================================================================

fn run_proptest_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_proptest_property);
    }
    let counter = Arc::new(AtomicU64::new(0));
    let t0 = Instant::now();
    let cfg = proptest::test_runner::Config {
        cases: cases_budget().min(u32::MAX as u64) as u32,
        max_shrink_iters: 32,
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(cfg);
    let c = counter.clone();
    let result: Result<(), String> = match property {
        "LeakInlinePanics" => runner
            .run(&leak_strategy(), move |v| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex = format!("({:?})", v);
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_leak_inline_panics(v.values.clone(), v.inline_cap)
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(cex)),
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        "AppendPreservesLength" => runner
            .run(&append_strategy(), move |v| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex = format!("({:?})", v);
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_append_preserves_length(v.a.clone(), v.b.clone())
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(cex)),
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        "FromVecZeroCapacity" => runner
            .run(&from_vec_zero_strategy(), move |v| {
                c.fetch_add(1, Ordering::Relaxed);
                let cex = format!("({:?})", v);
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_from_vec_zero_capacity(v.push_after.clone())
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => Ok(()),
                    Ok(PropertyResult::Fail(_)) | Err(_) => Err(TestCaseError::fail(cex)),
                }
            })
            .map_err(|e| match e {
                TestError::Fail(reason, _) => reason.to_string(),
                other => other.to_string(),
            }),
        _ => {
            return (
                Err(format!("Unknown property for proptest: {property}")),
                Metrics::default(),
            );
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = counter.load(Ordering::Relaxed);
    (result, Metrics { inputs, elapsed_us })
}

// ============================================================================
// quickcheck adapter (fork with `etna` feature — fn-pointer API)
// ============================================================================

static QC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn qc_leak_inline_panics(v: LeakInput) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        property_leak_inline_panics(v.values, v.inline_cap)
    }));
    match out {
        Ok(PropertyResult::Pass) => TestResult::passed(),
        Ok(PropertyResult::Discard) => TestResult::discard(),
        Ok(PropertyResult::Fail(_)) | Err(_) => TestResult::failed(),
    }
}

fn qc_append_preserves_length(v: AppendInput) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        property_append_preserves_length(v.a, v.b)
    }));
    match out {
        Ok(PropertyResult::Pass) => TestResult::passed(),
        Ok(PropertyResult::Discard) => TestResult::discard(),
        Ok(PropertyResult::Fail(_)) | Err(_) => TestResult::failed(),
    }
}

fn qc_from_vec_zero_capacity(v: FromVecZeroInput) -> TestResult {
    QC_COUNTER.fetch_add(1, Ordering::Relaxed);
    let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        property_from_vec_zero_capacity(v.push_after)
    }));
    match out {
        Ok(PropertyResult::Pass) => TestResult::passed(),
        Ok(PropertyResult::Discard) => TestResult::discard(),
        Ok(PropertyResult::Fail(_)) | Err(_) => TestResult::failed(),
    }
}

fn run_quickcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_quickcheck_property);
    }
    QC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let budget = cases_budget();
    let mut qc = QuickCheck::new()
        .tests(budget)
        .max_tests(budget.saturating_mul(4))
        .max_time(Duration::from_secs(86_400));
    let result = match property {
        "LeakInlinePanics" => qc.quicktest(qc_leak_inline_panics as fn(LeakInput) -> TestResult),
        "AppendPreservesLength" => {
            qc.quicktest(qc_append_preserves_length as fn(AppendInput) -> TestResult)
        }
        "FromVecZeroCapacity" => {
            qc.quicktest(qc_from_vec_zero_capacity as fn(FromVecZeroInput) -> TestResult)
        }
        _ => {
            return (
                Err(format!("Unknown property for quickcheck: {property}")),
                Metrics::default(),
            );
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = QC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        ResultStatus::Finished => Ok(()),
        ResultStatus::Failed { arguments } => Err(format!("({})", arguments.join(" "))),
        ResultStatus::Aborted { err } => Err(format!("quickcheck aborted: {err:?}")),
        ResultStatus::TimedOut => Err("quickcheck timed out".to_string()),
        ResultStatus::GaveUp => Err(format!(
            "quickcheck gave up after {} tests",
            result.n_tests_passed
        )),
    };
    (status, Metrics { inputs, elapsed_us })
}

// ============================================================================
// crabcheck adapter (fn-pointer API)
// ============================================================================

static CC_COUNTER: AtomicU64 = AtomicU64::new(0);

fn cc_leak_inline_panics(v: LeakInput) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_leak_inline_panics(v.values, v.inline_cap) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_append_preserves_length(v: AppendInput) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_append_preserves_length(v.a, v.b) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn cc_from_vec_zero_capacity(v: FromVecZeroInput) -> Option<bool> {
    CC_COUNTER.fetch_add(1, Ordering::Relaxed);
    match property_from_vec_zero_capacity(v.push_after) {
        PropertyResult::Pass => Some(true),
        PropertyResult::Fail(_) => Some(false),
        PropertyResult::Discard => None,
    }
}

fn run_crabcheck_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_crabcheck_property);
    }
    CC_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let cc_config = crabcheck_qc::Config {
        tests: cases_budget(),
    };
    let result = match property {
        "LeakInlinePanics" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_leak_inline_panics)
        }
        "AppendPreservesLength" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_append_preserves_length)
        }
        "FromVecZeroCapacity" => {
            crabcheck_qc::quickcheck_with_config(cc_config, cc_from_vec_zero_capacity)
        }
        _ => {
            return (
                Err(format!("Unknown property for crabcheck: {property}")),
                Metrics::default(),
            );
        }
    };
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = CC_COUNTER.load(Ordering::Relaxed);
    let status = match result.status {
        crabcheck_qc::ResultStatus::Finished => Ok(()),
        crabcheck_qc::ResultStatus::Failed { arguments } => {
            Err(format!("({})", arguments.join(" ")))
        }
        crabcheck_qc::ResultStatus::TimedOut => Err("crabcheck timed out".to_string()),
        crabcheck_qc::ResultStatus::GaveUp => Err(format!(
            "crabcheck gave up: passed={}, discarded={}",
            result.passed, result.discarded
        )),
        crabcheck_qc::ResultStatus::Aborted { error } => {
            Err(format!("crabcheck aborted: {error}"))
        }
    };
    (status, Metrics { inputs, elapsed_us })
}

// ============================================================================
// hegel adapter (real hegeltest 0.3.7 — panic-on-cex API)
// ============================================================================

static HG_COUNTER: AtomicU64 = AtomicU64::new(0);

fn hegel_settings() -> HegelSettings {
    HegelSettings::new()
        .test_cases(cases_budget())
        .suppress_health_check(HealthCheck::all())
}

fn hg_draw_u8(tc: &TestCase) -> u8 {
    tc.draw(hgen::integers::<u32>().min_value(0).max_value(255)) as u8
}

fn hg_draw_u16(tc: &TestCase) -> u16 {
    tc.draw(hgen::integers::<u32>().min_value(0).max_value(65535)) as u16
}

fn hg_draw_i64(tc: &TestCase) -> i64 {
    let hi = tc.draw(hgen::integers::<u32>().min_value(0).max_value(u32::MAX)) as u64;
    let lo = tc.draw(hgen::integers::<u32>().min_value(0).max_value(u32::MAX)) as u64;
    ((hi << 32) | lo) as i64
}

fn hg_draw_vec_u8(tc: &TestCase) -> Vec<u8> {
    let n = tc.draw(hgen::integers::<u32>().min_value(0).max_value(32)) as usize;
    (0..n).map(|_| hg_draw_u8(tc)).collect()
}

fn hg_draw_vec_i64(tc: &TestCase) -> Vec<i64> {
    let n = tc.draw(hgen::integers::<u32>().min_value(0).max_value(16)) as usize;
    (0..n).map(|_| hg_draw_i64(tc)).collect()
}

fn hg_draw_vec_u16(tc: &TestCase) -> Vec<u16> {
    let n = tc.draw(hgen::integers::<u32>().min_value(0).max_value(16)) as usize;
    (0..n).map(|_| hg_draw_u16(tc)).collect()
}

fn run_hegel_property(property: &str) -> Outcome {
    if property == "All" {
        return run_all(run_hegel_property);
    }
    HG_COUNTER.store(0, Ordering::Relaxed);
    let t0 = Instant::now();
    let settings = hegel_settings();
    let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match property {
        "LeakInlinePanics" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let values = hg_draw_vec_u8(&tc);
                let inline_cap = hg_draw_u8(&tc);
                let cex = format!("(values={:?} cap_tag={})", values, inline_cap);
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_leak_inline_panics(values.clone(), inline_cap)
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{}", cex),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "AppendPreservesLength" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let a = hg_draw_vec_i64(&tc);
                let b = hg_draw_vec_i64(&tc);
                let cex = format!("(a={:?} b={:?})", a, b);
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_append_preserves_length(a.clone(), b.clone())
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{}", cex),
                }
            })
            .settings(settings.clone())
            .run();
        }
        "FromVecZeroCapacity" => {
            Hegel::new(|tc: TestCase| {
                HG_COUNTER.fetch_add(1, Ordering::Relaxed);
                let push_after = hg_draw_vec_u16(&tc);
                let cex = format!("(push_after={:?})", push_after);
                let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    property_from_vec_zero_capacity(push_after.clone())
                }));
                match out {
                    Ok(PropertyResult::Pass) | Ok(PropertyResult::Discard) => {}
                    Ok(PropertyResult::Fail(_)) | Err(_) => panic!("{}", cex),
                }
            })
            .settings(settings.clone())
            .run();
        }
        _ => panic!("__unknown_property:{}", property),
    }));
    let elapsed_us = t0.elapsed().as_micros();
    let inputs = HG_COUNTER.load(Ordering::Relaxed);
    let metrics = Metrics { inputs, elapsed_us };
    let status = match run_result {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "hegel panicked with non-string payload".to_string()
            };
            if let Some(rest) = msg.strip_prefix("__unknown_property:") {
                return (
                    Err(format!("Unknown property for hegel: {rest}")),
                    Metrics::default(),
                );
            }
            Err(msg
                .strip_prefix("Property test failed: ")
                .unwrap_or(&msg)
                .to_string())
        }
    };
    (status, metrics)
}

// ============================================================================
// dispatch + main
// ============================================================================

fn run(tool: &str, property: &str) -> Outcome {
    match tool {
        "etna" => run_etna_property(property),
        "proptest" => run_proptest_property(property),
        "quickcheck" => run_quickcheck_property(property),
        "crabcheck" => run_crabcheck_property(property),
        "hegel" => run_hegel_property(property),
        _ => (Err(format!("Unknown tool: {tool}")), Metrics::default()),
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn emit_json(
    tool: &str,
    property: &str,
    status: &str,
    metrics: Metrics,
    counterexample: Option<&str>,
    error: Option<&str>,
) {
    let cex = counterexample.map_or("null".to_string(), json_str);
    let err = error.map_or("null".to_string(), json_str);
    println!(
        "{{\"status\":{},\"tests\":{},\"discards\":0,\"time\":{},\"counterexample\":{},\"error\":{},\"tool\":{},\"property\":{}}}",
        json_str(status),
        metrics.inputs,
        json_str(&format!("{}us", metrics.elapsed_us)),
        cex,
        err,
        json_str(tool),
        json_str(property),
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <tool> <property>", args[0]);
        eprintln!("Tools: etna | proptest | quickcheck | crabcheck | hegel");
        eprintln!(
            "Properties: LeakInlinePanics | AppendPreservesLength | FromVecZeroCapacity | All"
        );
        std::process::exit(2);
    }
    let (tool, property) = (args[1].as_str(), args[2].as_str());

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(tool, property)));
    std::panic::set_hook(previous_hook);

    let (result, metrics) = match caught {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "panic with non-string payload".to_string()
            };
            emit_json(tool, property, "aborted", Metrics::default(), None, Some(&msg));
            return;
        }
    };

    match result {
        Ok(()) => emit_json(tool, property, "passed", metrics, None, None),
        Err(e) => emit_json(tool, property, "failed", metrics, Some(&e), None),
    }
}

#!/bin/bash
set -euo pipefail

usage() {
echo 'Usage: ./steps.sh [PARAMS]

Params:
  --workload_path <VALUE>
  --strategy <VALUE>
  --tests <VALUE>
  --property <VALUE>
  --stages <LIST>  (optional; comma-separated: check,build,run or "all")
  -h, --help       Show this help and exit'
}

# Initialize variables
workload_path=""
strategy=""
tests=""
property=""
STAGES=""

# Parse keyword args
while [ $# -gt 0 ]; do
  case "$1" in
    --workload_path=*) workload_path="${1#*=}"; shift ;;
    --workload_path) shift; [ $# -gt 0 ] || { echo "Missing value for --workload_path" >&2; usage; exit 2; }; workload_path="$1"; shift ;;
    --strategy=*) strategy="${1#*=}"; shift ;;
    --strategy) shift; [ $# -gt 0 ] || { echo "Missing value for --strategy" >&2; usage; exit 2; }; strategy="$1"; shift ;;
    --tests=*) tests="${1#*=}"; shift ;;
    --tests) shift; [ $# -gt 0 ] || { echo "Missing value for --tests" >&2; usage; exit 2; }; tests="$1"; shift ;;
    --property=*) property="${1#*=}"; shift ;;
    --property) shift; [ $# -gt 0 ] || { echo "Missing value for --property" >&2; usage; exit 2; }; property="$1"; shift ;;
    --stages=*) STAGES="${1#*=}"; shift ;;
    --stages)   shift; [ $# -gt 0 ] || { echo "Missing value for --stages" >&2; usage; exit 2; }; STAGES="$1"; shift ;;
    --) shift; break ;;
    -h|--help) usage; exit 0 ;;
    --*) echo "Unknown option: $1" >&2; usage; exit 2 ;;
    *) break ;;
  esac
done

# Enforce required vars
[ -n "$workload_path" ] || { echo "Missing required option: --workload_path" >&2; usage; exit 2; }
[ -n "$strategy" ] || { echo "Missing required option: --strategy" >&2; usage; exit 2; }
[ -n "$tests" ] || { echo "Missing required option: --tests" >&2; usage; exit 2; }
[ -n "$property" ] || { echo "Missing required option: --property" >&2; usage; exit 2; }

# Compute requested stages (default: all)
if [ -z "$STAGES" ]; then
  STAGES="all"
fi

# Normalize, validate, and store selection (portable: no associative arrays)
W_CHECK=0
W_BUILD=0
W_RUN=0

if [ "$STAGES" = "all" ]; then
  W_CHECK=1
  W_BUILD=1
  W_RUN=1
else
  IFS=',' read -r -a __requested_stages <<< "$STAGES"
  for s in "${__requested_stages[@]}"; do
    s="$(printf '%s' "$s" | tr '[:upper:]' '[:lower:]' | xargs)"
    case "$s" in
      check) W_CHECK=1 ;;
      build) W_BUILD=1 ;;
      run)   W_RUN=1 ;;
      "" )   ;; # ignore empties
      * )    echo "Unknown stage: $s" >&2; usage; exit 2 ;;
    esac
  done
fi

# Ensure at least one stage selected
if [ $((W_CHECK + W_BUILD + W_RUN)) -eq 0 ]; then
  echo "No valid stages selected (got: $STAGES)" >&2
  usage
  exit 2
fi

# Build a human-readable list
__list=""
[ $W_CHECK -eq 1 ] && __list="${__list}check "
[ $W_BUILD -eq 1 ] && __list="${__list}build "
[ $W_RUN  -eq 1 ] && __list="${__list}run"
echo "[steps.sh] Stages to run: $__list" >&2

# Export for children
export workload_path
export strategy
export tests
export property

echo "[steps.sh] Effective options:" >&2

# ===== Check Steps =====
if [ $W_CHECK -eq 1 ]; then
    # Required tools (install if missing):
    #   - crabcheck-profiling-fast-analyze: `cargo install --path $CRABCHECK_DIR --bin crabcheck-profiling-fast-analyze --features profiling`
    #   - llvm-profdata / llvm-cov: shipped with rustc's llvm-tools-preview component (run `rustup component add llvm-tools-preview`) or via `brew install llvm`.
    for bin in crabcheck-profiling-fast-analyze llvm-profdata llvm-cov; do
      command -v "$bin" >/dev/null 2>&1 || echo "⚠️  missing: $bin"
    done

  echo 'Check steps are completed.'
fi

# ===== Build Steps =====
if [ $W_BUILD -eq 1 ]; then
(cd ${workload_path} &&     CARGO_INCREMENTAL="0" RUSTFLAGS="-C instrument-coverage -C link-dead-code -C codegen-units=1 -C inline-threshold=0 -C llvm-args=-inline-threshold=0 -C debuginfo=2" cargo build --release)

  echo 'Build steps are completed.'
fi

# ===== Run Steps =====
if [ $W_RUN -eq 1 ]; then
(cd ${workload_path} &&     mkdir -p coverage &&     LLVM_PROFILE_FILE="coverage/snapshot_%p-%m.profraw" ./target/release/etna-faultloc ${strategy} ${property} ${tests})

(cd ${workload_path} &&     crabcheck-profiling-fast-analyze coverage smallvec ./target/release/etna-faultloc)

    echo 'Run steps are completed.'
fi
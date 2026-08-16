#!/usr/bin/env bash
set -euo pipefail

workspace="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly workspace
readonly manifest="$workspace/tools/coverage/shards.txt"
readonly generated="$workspace/bazel-out/_coverage/_coverage_report.dat"

if [[ -n "${LOOPAL_COVERAGE_OUTPUT_DIR:-}" ]]; then
  mkdir -p -- "$LOOPAL_COVERAGE_OUTPUT_DIR"
  output_dir="$(cd "$LOOPAL_COVERAGE_OUTPUT_DIR" && pwd -P)"
else
  output_dir="$(mktemp -d "${TMPDIR:-/tmp}/loopal-coverage.XXXXXX")"
fi
readonly output_dir

declare -a base_reports=()
declare -a branch_reports=()
current_name=""
current_mode=""
current_jobs=""
current_threads=""
declare -a current_targets=()

run_report() {
  local name="$1"
  local mode="$2"
  local jobs="$3"
  local threads="$4"
  shift 4
  local destination="$output_dir/$name.lcov"
  local -a args=(
    coverage
    "--jobs=$jobs"
    --local_test_jobs=1
    --combined_report=lcov
    --test_output=errors
  )
  if [[ "$mode" == "branch" ]]; then
    args+=(--config=rust_branch)
  fi
  if [[ "$threads" != "none" ]]; then
    args+=("--test_arg=--test-threads=$threads")
  fi

  rm -f -- "$generated"
  echo "==> coverage producer: $name"
  bazel "${args[@]}" "$@"
  if [[ ! -s "$generated" ]]; then
    echo "coverage producer did not publish a non-empty LCOV report: $name" >&2
    exit 1
  fi
  cp -- "$generated" "$destination"
  if [[ "$mode" == "base" ]]; then
    base_reports+=("$destination")
  else
    branch_reports+=("$destination")
  fi
}

flush_shard() {
  if [[ -z "$current_name" ]]; then
    return
  fi
  run_report \
    "$current_name" "$current_mode" "$current_jobs" "$current_threads" \
    "${current_targets[@]}"
  current_targets=()
}

validate_manifest() {
  awk -F'|' '
    BEGIN { base = 0; branch = 0; previous = "" }
    /^#/ || NF == 0 { next }
    NF != 5 { print "invalid field count at line " NR > "/dev/stderr"; exit 1 }
    $2 != "base" && $2 != "branch" {
      print "invalid mode at line " NR > "/dev/stderr"; exit 1
    }
    $3 !~ /^[1-9][0-9]*$/ {
      print "invalid jobs value at line " NR > "/dev/stderr"; exit 1
    }
    $4 != "none" && $4 !~ /^[1-9][0-9]*$/ {
      print "invalid test_threads at line " NR > "/dev/stderr"; exit 1
    }
    $5 !~ /^\/\// {
      print "invalid target at line " NR > "/dev/stderr"; exit 1
    }
    $1 != previous {
      if (seen[$1]++) {
        print "non-contiguous shard at line " NR > "/dev/stderr"; exit 1
      }
      if ($2 == "base") base++; else branch++
      previous = $1
      mode = $2; jobs = $3; threads = $4
    }
    $2 != mode || $3 != jobs || $4 != threads {
      print "inconsistent shard metadata at line " NR > "/dev/stderr"; exit 1
    }
    END {
      if (base != 5 || branch != 11) {
        print "manifest must contain 5 base and 11 branch shards" > "/dev/stderr"
        exit 1
      }
    }
  ' "$manifest"
}

cd "$workspace"
echo "coverage output directory: $output_dir"
validate_manifest
bazel run //tools/coverage:scope_review

while IFS='|' read -r name mode jobs threads target || [[ -n "$name" ]]; do
  if [[ -z "$name" || "$name" == \#* ]]; then
    continue
  fi
  if [[ "$mode" != "base" && "$mode" != "branch" ]]; then
    echo "invalid coverage shard mode for $name: $mode" >&2
    exit 1
  fi
  if [[ -z "$target" || "$target" != //* ]]; then
    echo "invalid coverage target for $name: $target" >&2
    exit 1
  fi
  if [[ "$name" != "$current_name" ]]; then
    flush_shard
    current_name="$name"
    current_mode="$mode"
    current_jobs="$jobs"
    current_threads="$threads"
  elif [[ "$mode|$jobs|$threads" != "$current_mode|$current_jobs|$current_threads" ]]; then
    echo "inconsistent metadata inside coverage shard: $name" >&2
    exit 1
  fi
  current_targets+=("$target")
done < "$manifest"
flush_shard

if [[ "${#base_reports[@]}" -ne 5 || "${#branch_reports[@]}" -ne 11 ]]; then
  echo "coverage manifest must produce exactly 5 base and 11 branch reports" >&2
  exit 1
fi

readonly combined_base="$output_dir/base.lcov"
cp -- "${base_reports[0]}" "$combined_base"
chmod u+w "$combined_base"
for report in "${base_reports[@]:1}"; do
  cat "$report" >> "$combined_base"
done

bazel run //tools/coverage:gate -- "$combined_base" "${branch_reports[@]}"
echo "coverage reports retained in $output_dir"

#!/usr/bin/env bash
set -euo pipefail

# Coverage matrix leveraging cargo-tarpaulin. Each entry lists feature flags.
MATRIX=(
  "--no-default-features --features standard64 std"
  "--no-default-features --features compact48 std"
  "--features standard64 std serde"
  "--features standard64 std mpmc"
  "--features standard64 std serde mpmc"
)

report_dir="target/tarpaulin"
mkdir -p "$report_dir"

for opts in "${MATRIX[@]}"; do
  echo "==> Running tarpaulin ${opts}" >&2
  cargo tarpaulin ${opts} \
    --out Xml \
    --output-dir "$report_dir" \
    --timeout 120 || {
      echo "Tarpaulin failed for options: ${opts}" >&2
      exit 1
    }
  echo "" >&2
  sleep 1

done

echo "All coverage runs completed. Consolidated reports in ${report_dir}" >&2

#!/usr/bin/env bash
# differential-test.sh — Gate 2 differential compile (Rust vs Python).
#
# Stub. Real implementation lands in Gate 2 (see spec § 19 Gate 2,
# § 4.5 platform-repo access).
#
# When implemented this script must:
#   - resolve ${PLATFORM_REPO:-../institutional-defi-platform-api}
#   - verify the platform checkout's HEAD matches fixtures/rules/SOURCE.md
#   - for every YAML in fixtures/rules/, run the Python compiler against
#     the resolved platform checkout AND run ke-compiler
#   - normalize and diff the resulting IRs
#   - exit non-zero on any divergence

set -euo pipefail

echo "stub: differential-test.sh — Gate 2 has not started." >&2
echo "see docs/spec/ke-workbench-rust-migration-spec-v3.1.md § 19 Gate 2." >&2
exit 64  # EX_USAGE

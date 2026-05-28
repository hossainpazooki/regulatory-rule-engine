#!/usr/bin/env bash
# equivalence-harness.sh — Gate 3 fuzzed Rust vs Python trace equivalence.
#
# Stub. Real implementation lands in Gate 3 (see spec § 19 Gate 3,
# § 4.5 platform-repo access, § 20 duplicate-runtime drift).
#
# When implemented this script must:
#   - resolve ${PLATFORM_REPO:-../institutional-defi-platform-api}
#   - verify the platform checkout's HEAD matches fixtures/rules/SOURCE.md
#   - generate N scenarios via property-based / metamorphic strategies
#   - execute each scenario through the Python RuleRuntime AND ke-runtime
#   - compare normalized public trace events + outcomes + obligation sets +
#     error classes (the equivalence boundary in spec § 20)
#   - record the platform commit SHA in the run output

set -euo pipefail

echo "stub: equivalence-harness.sh — Gate 3 has not started." >&2
echo "see docs/spec/ke-workbench-rust-migration-spec-v3.1.md § 19 Gate 3." >&2
exit 64  # EX_USAGE

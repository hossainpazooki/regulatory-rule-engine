#!/usr/bin/env bash
# bootstrap.sh — snapshot the platform rule corpus into fixtures/rules/.
#
# Per spec § 4.5 (platform-repo access model) and § 19 Gate 0.
# This is the *documented sync script* mentioned in spec § 4.4; ordinary
# editing of fixtures/ is otherwise forbidden inside actor sessions.
#
# Resolution: ${PLATFORM_REPO:-../institutional-defi-platform-api}
# Fails fast if the platform checkout is missing or if files under
# src/rules/data/ are modified or untracked.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
platform="${PLATFORM_REPO:-$here/../institutional-defi-platform-api}"
platform_rel_data="src/rules/data"

# --- preconditions ---------------------------------------------------------

if [[ ! -d "$platform" ]]; then
  echo "error: platform repo not found at: $platform" >&2
  echo "       set PLATFORM_REPO to override, or place it as a sibling of ke-workbench." >&2
  exit 1
fi

if [[ ! -d "$platform/.git" ]]; then
  echo "error: $platform is not a git checkout" >&2
  exit 1
fi

if [[ ! -d "$platform/$platform_rel_data" ]]; then
  echo "error: missing $platform_rel_data inside $platform" >&2
  exit 1
fi

# Reject a dirty rule corpus in the platform checkout so the recorded SHA
# actually identifies the bytes we copied.
dirty="$(git -C "$platform" status --porcelain -- "$platform_rel_data")"
if [[ -n "$dirty" ]]; then
  echo "error: platform $platform_rel_data is dirty; commit or stash first:" >&2
  echo "$dirty" >&2
  exit 1
fi

platform_sha="$(git -C "$platform" rev-parse HEAD)"
platform_short="$(git -C "$platform" rev-parse --short HEAD)"
platform_origin="$(git -C "$platform" remote get-url origin 2>/dev/null || echo '(no origin)')"
copy_ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# --- snapshot --------------------------------------------------------------

dest="$here/fixtures/rules"
mkdir -p "$dest"

# Wipe everything except the SOURCE.md we are about to write and the .gitkeep
# so stale fixtures cannot survive a rules-data rename in the platform.
find "$dest" -mindepth 1 \
  ! -name 'SOURCE.md' \
  ! -name '.gitkeep' \
  -exec rm -rf {} +

cp -R "$platform/$platform_rel_data/." "$dest/"

# --- provenance record -----------------------------------------------------

cat > "$dest/SOURCE.md" <<EOF
# fixtures/rules/ — provenance

Snapshotted from \`institutional-defi-platform-api/$platform_rel_data\` by
\`scripts/bootstrap.sh\`. Do not edit by hand — re-run the script to refresh.

| Field             | Value                                |
| ----------------- | ------------------------------------ |
| Platform repo     | \`$platform_origin\`                 |
| Platform commit   | \`$platform_sha\`                    |
| Platform short    | \`$platform_short\`                  |
| Snapshot taken at | $copy_ts                             |
| Snapshot host     | $(uname -a 2>/dev/null || echo n/a)  |

Per spec § 4.5, subsequent gate scripts (\`differential-test.sh\`,
\`equivalence-harness.sh\`) must verify the platform checkout still points at
this exact commit, or the gate run is invalid.
EOF

echo "ok: snapshotted $platform_rel_data @ $platform_short into fixtures/rules/"

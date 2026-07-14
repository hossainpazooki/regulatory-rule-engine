#!/usr/bin/env bash
# ADR-0023 differential harness — NON-GATING (deliberately not in CI; the
# Playwright precedent). Run on demand; the PR that closes the build trigger
# records this script's green run as the claim gate.
#
# What it proves (D6/D7 of the ADR):
#   1. Two Cypher queries over a Neo4j load of `ke graph export` output agree
#      EXACTLY with the pure-Rust oracles (`ke graph oracle-*`) computed from
#      the same verified registry — with a VACUITY GUARD: both sides must be
#      non-empty before equality counts (an all-refused export or an empty
#      oracle is a red run, never a trivially-equal green one — the R7 lesson,
#      ADR-0022 § Context).
#   2. Negative control (a): a twin export with ONE mutated CONFLICTS_WITH
#      endpoint makes the Cypher side DISAGREE with the oracle — the
#      differential actually discriminates.
#   3. Negative control (b): a twin export with ONE doctored attestation type
#      makes the kind-policy violation query return non-empty (it is empty on
#      the honest export) — the ADR-0022 kind asymmetry is enforced visibly.
#
# Needs: docker (Neo4j 5 Community, auth disabled), cargo. Roughly 2-5 min.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d)"
REG="$WORK/registry"
OUT="$WORK/out"
NOW=1750000000
COMPOSE="docker compose -f $ROOT/scripts/graph-differential/docker-compose.yml"
CONTAINER=ke-graph-differential

PASS=0
FAIL=0
step() { printf '\n== %s\n' "$*"; }
ok()   { PASS=$((PASS+1)); printf 'PASS: %s\n' "$*"; }
die()  { printf 'FAIL: %s\n' "$*"; FAIL=$((FAIL+1)); }

cleanup() {
  $COMPOSE down -v >/dev/null 2>&1 || true
  rm -rf "$WORK"
}
trap cleanup EXIT

cd "$ROOT"

step "build ke (test-keys)"
cargo build -q -p ke-cli --features test-keys
KE="$ROOT/target/debug/ke"

# ---------------------------------------------------------------------------
# Seed a registry: two harness packs (cross-regime shared citation + an
# intra-pack publishable conflict), one real corpus pack, one IntentSpec.
# ---------------------------------------------------------------------------
step "seed + publish registry"
publish_pack() { # yaml regime
  local hash
  hash=$("$KE" --registry "$REG" --now $NOW compile "$1" --regime "$2" \
    | sed -n 's/^compiled: hash=\([0-9a-f]*\).*/\1/p')
  [ -n "$hash" ] || { echo "no hash from compile $1"; exit 1; }
  "$KE" --registry "$REG" --now $NOW ml-check --hash "$hash" >/dev/null
  "$KE" --registry "$REG" --now $NOW attest --hash "$hash" \
    --type source_fidelity --type scenario_coverage --type publication_approval >/dev/null
  "$KE" --registry "$REG" --now $NOW publish --hash "$hash" --env staging --tag "$2" >/dev/null
  echo "$hash"
}

HASH_A=$(publish_pack crates/ke-cli/tests/fixtures/graph/regime_a.yaml regime_a)
HASH_B=$(publish_pack crates/ke-cli/tests/fixtures/graph/regime_b.yaml regime_b)
HASH_MICA=$(publish_pack fixtures/rules/mica_stablecoin.yaml mica_2023)

INTENT_HASH=$("$KE" --registry "$REG" --now $NOW compile-intent \
  scripts/graph-differential/intent_treasury.json --regime treasury_payments_v1 \
  | sed -n 's/.*hash=\([0-9a-f]*\).*/\1/p')
"$KE" --registry "$REG" --now $NOW ml-check --hash "$INTENT_HASH" >/dev/null
"$KE" --registry "$REG" --now $NOW attest --hash "$INTENT_HASH" \
  --type source_fidelity --type publication_approval >/dev/null
"$KE" --registry "$REG" --now $NOW publish --hash "$INTENT_HASH" --env staging \
  --tag treasury >/dev/null
echo "published: A=$HASH_A B=$HASH_B mica=$HASH_MICA intent=$INTENT_HASH"

# Plus one artifact deliberately left unpublished: the refusal log must name it.
UNPUB=$("$KE" --registry "$REG" --now $NOW compile fixtures/rules/fca_crypto.yaml \
  --regime fca_crypto_2024 | sed -n 's/^compiled: hash=\([0-9a-f]*\).*/\1/p')

step "export graph (verify-gated)"
"$KE" --registry "$REG" --now $NOW graph export --out "$OUT" --env local
grep -q "$UNPUB.*not published" "$OUT/refusals.log" \
  && ok "unpublished artifact refused by name" \
  || die "refusal log does not name the unpublished artifact"
grep -q "$UNPUB" "$OUT/graph.cypher" \
  && die "refused artifact leaked into the graph" \
  || ok "refused artifact absent from the graph"
grep -q "CONFLICTS_WITH" "$OUT/graph.cypher" \
  && ok "conflict edges present in export" \
  || die "no CONFLICTS_WITH edge in export (vacuity)"
grep -q "IntentSpec" "$OUT/graph.cypher" \
  && ok "IntentSpec node family present in export" \
  || die "no IntentSpec in export (vacuity)"

# ---------------------------------------------------------------------------
# Rust-side oracles.
# ---------------------------------------------------------------------------
step "rust oracles"
"$KE" --registry "$REG" --now $NOW graph oracle-blast --doc doc_shared \
  | sort > "$WORK/oracle_blast.txt"
"$KE" --registry "$REG" --now $NOW graph oracle-exposure --from regime_a --to regime_b \
  | sort > "$WORK/oracle_exposure.txt"
[ -s "$WORK/oracle_blast.txt" ]    && ok "blast oracle non-empty (vacuity guard)" \
                                   || die "blast oracle EMPTY (vacuity)"
[ -s "$WORK/oracle_exposure.txt" ] && ok "exposure oracle non-empty (vacuity guard)" \
                                   || die "exposure oracle EMPTY (vacuity)"

# ---------------------------------------------------------------------------
# Neo4j: load and differential-test.
# ---------------------------------------------------------------------------
step "start neo4j"
$COMPOSE up -d
for i in $(seq 1 60); do
  if docker exec "$CONTAINER" cypher-shell "RETURN 1;" >/dev/null 2>&1; then break; fi
  [ "$i" = 60 ] && { echo "neo4j did not become ready"; exit 1; }
  sleep 3
done
echo "neo4j ready"

load_cypher() { # file
  docker exec -i "$CONTAINER" cypher-shell --format plain \
    "MATCH (n) DETACH DELETE n;" >/dev/null
  docker exec -i "$CONTAINER" cypher-shell --format plain \
    < "$1" >/dev/null
}

cypher_blast() {
  docker exec -i "$CONTAINER" cypher-shell --format plain <<'EOF' | tail -n +2 | tr -d '"' | sort
MATCH (n)-[c:CITES|SPANS]->(:CorpusDoc {id: 'doc_shared'})
WITH collect(DISTINCT n) AS citing
UNWIND citing AS n
MATCH (a:Artifact)-[:CONTAINS]->(n)
OPTIONAL MATCH (a)-[:ATTESTED_BY]->(t:Attestation)
WITH collect(DISTINCT n.id) + collect(DISTINCT a.id) + collect(DISTINCT t.id) AS ids
UNWIND ids AS id
RETURN DISTINCT id ORDER BY id;
EOF
}

cypher_exposure() {
  docker exec -i "$CONTAINER" cypher-shell --format plain <<'EOF' | tail -n +2 | tr -d '"' | sort
MATCH (src:Rule)<-[:CONTAINS]-(:Artifact)-[:IN_REGIME]->(:Regime {id: 'regime_a'})
MATCH (src)-[:CITES|SPANS*0..]-(b:Rule)
MATCH (b)-[:CONFLICTS_WITH]-(c:Rule)<-[:CONTAINS]-(:Artifact)-[:IN_REGIME]->(:Regime {id: 'regime_b'})
RETURN DISTINCT c.id ORDER BY c.id;
EOF
}

cypher_policy_violations() {
  docker exec -i "$CONTAINER" cypher-shell --format plain <<'EOF' | tail -n +2 | tr -d '"' | sort
MATCH (a:Artifact)
WITH a, [(a)-[:ATTESTED_BY]->(t) | t.type] AS types,
     CASE a.kind
       WHEN 'IntentSpec' THEN ['SourceFidelity', 'PublicationApproval']
       ELSE ['SourceFidelity', 'ScenarioCoverage', 'PublicationApproval']
     END AS required
WHERE any(r IN required WHERE NOT r IN types)
RETURN a.id ORDER BY a.id;
EOF
}

step "differential: honest export"
load_cypher "$OUT/graph.cypher"
cypher_blast > "$WORK/cypher_blast.txt"
cypher_exposure > "$WORK/cypher_exposure.txt"
[ -s "$WORK/cypher_blast.txt" ] || die "cypher blast EMPTY (vacuity)"
[ -s "$WORK/cypher_exposure.txt" ] || die "cypher exposure EMPTY (vacuity)"
diff "$WORK/oracle_blast.txt" "$WORK/cypher_blast.txt" >/dev/null \
  && ok "blast-radius differential: Cypher == Rust oracle" \
  || { die "blast-radius differential MISMATCH"; diff "$WORK/oracle_blast.txt" "$WORK/cypher_blast.txt" || true; }
diff "$WORK/oracle_exposure.txt" "$WORK/cypher_exposure.txt" >/dev/null \
  && ok "conflict-exposure differential: Cypher == Rust oracle" \
  || { die "conflict-exposure differential MISMATCH"; diff "$WORK/oracle_exposure.txt" "$WORK/cypher_exposure.txt" || true; }
VIOLATIONS=$(cypher_policy_violations)
[ -z "$VIOLATIONS" ] \
  && ok "kind-policy violation query empty on honest export" \
  || die "honest export shows policy violations: $VIOLATIONS"

step "negative control (a): mutated conflict edge must break the differential"
sed "s/\(MERGE (a)-\[r:CONFLICTS_WITH\]->(b)\)/\1 SET r.mutated='1'/; s/:beta_two'}) MERGE (a)-\[r:CONFLICTS_WITH\]/:beta_one'}) MERGE (a)-[r:CONFLICTS_WITH]/" \
  "$OUT/graph.cypher" > "$WORK/twin_mutated.cypher"
if diff -q "$OUT/graph.cypher" "$WORK/twin_mutated.cypher" >/dev/null; then
  die "mutation did not change the twin (control is vacuous)"
else
  load_cypher "$WORK/twin_mutated.cypher"
  cypher_exposure > "$WORK/cypher_exposure_mutated.txt"
  diff "$WORK/oracle_exposure.txt" "$WORK/cypher_exposure_mutated.txt" >/dev/null \
    && die "mutated twin still matches the oracle — differential is BLIND" \
    || ok "mutated twin detected (differential went red as required)"
fi

step "negative control (b): doctored attestation must trip the policy query"
sed "0,/SET n.type = 'PublicationApproval'/s//SET n.type = 'Interpretation'/" \
  "$OUT/graph.cypher" > "$WORK/twin_doctored.cypher"
if diff -q "$OUT/graph.cypher" "$WORK/twin_doctored.cypher" >/dev/null; then
  die "doctoring did not change the twin (control is vacuous)"
else
  load_cypher "$WORK/twin_doctored.cypher"
  DOCTORED=$(cypher_policy_violations)
  [ -n "$DOCTORED" ] \
    && ok "doctored attestation set detected: $DOCTORED" \
    || die "doctored twin passes the kind-policy query — control is BLIND"
fi

step "result"
echo "passed=$PASS failed=$FAIL"
if [ "$FAIL" -gt 0 ]; then
  echo "GRAPH DIFFERENTIAL: RED"
  exit 1
fi
echo "GRAPH DIFFERENTIAL: GREEN"

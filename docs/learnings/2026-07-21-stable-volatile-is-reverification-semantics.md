ts: 2026-07-21T14:10:28Z
commit: 151036a (branch docs/adr-0024-acceptance-stamp)
session: 7f20dfba-7a07-4c11-a7e7-5be8c9e7d0af
status: refuted-assumption

fact: The IntentSpec criterion tag `stable|volatile` is RE-VERIFICATION
semantics, not an asset classification. Stable = scored once at declaration;
volatile = scored at declaration AND re-verified at the dispatch edge by the
same gate, where a non-Pass ⟹ `FAILED_AT_DISPATCH` and nothing settles. The
2026-07-20 regulation-research prompt mischaracterized it as a
stablecoin-vs-volatile-token asset tag; under the correct reading, the
research's IPR finding (sanctions screening refreshed at least daily) is the
cleanest fit in the run — a volatile criterion with a freshness bound.
Assumption refuted: mine, in the research prompt; the finding landed
2026-07-20 while grounding the bank-vs-corporate treasury answer in
CONTRACT.md.

basis: `grep -n -A3 'Stable vs volatile'
../treasury-intent-controller/CONTRACT.md` (re-captured
2026-07-21T14:10:28Z) → "3. **Stable vs volatile.** Stable criteria scored
once (at declaration). Volatile criteria scored at declaration AND
re-verified at the dispatch edge by the SAME gate before authorizing. A
volatile criterion that is not `Pass` at re-verify ⟹ `FAILED_AT_DISPATCH`,
nothing dispatches."

re-verify: grep -A3 'Stable vs volatile' ../treasury-intent-controller/CONTRACT.md

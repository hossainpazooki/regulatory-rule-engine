ts: 2026-07-21T14:10:29Z
commit: 151036a (branch docs/adr-0024-acceptance-stamp; origin/main 53501bd)
session: 7f20dfba-7a07-4c11-a7e7-5be8c9e7d0af
status: verified

fact: Committing follow-up work onto an already-squash-merged gate branch
strands it — no open PR carries it, and main silently diverges from the
author's intent. Concretely: the ADR-0024 acceptance stamp was committed as
`1d556b8` onto `migration/gate-6-revocation-decision` after PR #17 had
squash-merged that branch; for ~1 day `origin/main` misrepresented ADR-0024
as "Proposed" while the stamp sat pushed-but-unreachable-from-main. Fixed by
re-carrying the change on a fresh branch off main (PR #18, merged as
`53501bd`). The stranded commit still exists on the stale remote branch.

basis: Captured 2026-07-20 at origin/main `7f2c99a` (pre-dates this entry's
anchor): `git branch -r --contains 1d556b8` → only
`origin/migration/gate-6-revocation-decision`; `git show
origin/main:docs/adr/0024-gate6-scope-reconciliation.md | sed -n 3p` →
"**Status:** Proposed (acceptance = PR merge, per the ADR-0023 precedent)".
Re-captured 2026-07-21T14:10:29Z at anchor: `git branch -r --contains
1d556b8` → still only the gate branch; `git log --oneline -1 origin/main` →
"53501bd docs(adr): stamp 0024 Accepted at merge (PR #17); Gate 6 closed
(#18)" (fix landed).

re-verify: git branch -r --contains 1d556b8   # must NOT be relied on to reach main; origin/main must carry the stamp independently (git log --oneline -3 origin/main)

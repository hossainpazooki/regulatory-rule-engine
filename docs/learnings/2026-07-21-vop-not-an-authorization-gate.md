ts: 2026-07-21T14:09:58Z
commit: 151036a (branch docs/adr-0024-acceptance-stamp)
session: 7f20dfba-7a07-4c11-a7e7-5be8c9e7d0af
status: verified

fact: Verification of Payee under the EU Instant Payments Regulation
(Regulation (EU) 2024/886) is NOT an execution-blocking authorization gate —
it is an inform-the-payer duty; the payer may proceed after any outcome. In
the 2026-07-20 adversarial research run, three independently-worded claims
that VoP is a "pre-execution/pre-authorization gate" were each refuted 0-3,
while the claims that VoP returns a categorical outcome (match / close match
/ no match / other) survived 3-0. Consequence for any future IPR
codification: model VoP as advisory data, never as a blocking IntentSpec
criterion; the IPR obligation that DOES fit the gate is the
at-least-daily customer sanctions-screening freshness rule (verified 3-0).

basis: Research workflow output (session task wxg714o2w, file
`.../tasks/wxg714o2w.output`, greped 2026-07-21T14:09:58Z), refuted array:
line 152 `"claim": "The IPR creates a mandatory pre-execution Verification
of Payee (VoP) check ... a payment-authorization gate ..."` with line 153
`"vote": "0-3"`; line 192 `"... it is a pre-authorization gate, not a
disclosure obligation."` also 0-3; six 0-3 votes total in the refuted set.
NOTE: the output file is session-scratchpad and will not persist; the quoted
rows above are the durable record.

re-verify: WebFetch https://www.ecb.europa.eu/paym/retail/instant_payments/html/instant_payments_regulation.en.html and confirm VoP is described as a service informing the payer (who may still authorize), not an execution block

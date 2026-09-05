# Tier 3 — deductive proof assistants (optional / future)

This directory is scaffolding for machine-checked deductive proofs. It is NOT on
the critical path and produces no build artefact today. See ../README.md for how
it relates to the active TLA+ and Kani tiers.

- `fstar/`  — **near-term candidate.** Verified signature-checking for the Secure
  Boot path (report §3.13). F\* proofs extract to C and can be linked into the
  `security` crate's signature module, exactly as HACL*/EverCrypt does for
  Firefox and the Linux kernel.
- `rocq/`   — **stretch goal.** Deductive proofs of algorithm-level properties
  (e.g. the deterministic policy engine always returns an explainable choice).
  This is the seL4/CompCert-scale ambition named as future work (report §6.6).

Nothing here is required to build or run MyBoot.

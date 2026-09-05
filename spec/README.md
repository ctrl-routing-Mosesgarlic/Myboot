# Formal specification & verification

MyBoot uses **three tiers of assurance** (report §2.6.8, §3.11–3.12, §5.7, §6.6).
Only the first two are active; the third is optional / future work and is
scaffolded here so it has a home.

| Tier | Tool | Kind | Target | Status |
|------|------|------|--------|--------|
| 1 | **TLA+** (`tla/`) | specification + model checking (TLC) | the boot transaction (design level) | **active** |
| 2 | **Kani** (`kani/`) | bounded model checking of Rust | the parsers (code level) | **active** |
| 3 | **Proof assistants** (`proofs/`) | deductive machine-checked proof | crypto (near-term) / whole-system (stretch) | **optional / future** |

## Why this split
Tiers 1–2 give reproducible, automated results on every build and were chosen
(over Coq/Rocq, Lean, Agda, Idris, Alloy, ACL2) as the best fit for a project of
this scope. A full deductive proof of the whole system, in the manner of seL4
(Isabelle) or CompCert (Rocq), is a person-years undertaking and is deliberately
future work (report §6.5–6.6).

## The one near-term proof-assistant candidate
The Secure Boot **signature-verification** path (report §3.13) is the single
subsystem where deductive, verified cryptography is warranted. **F\*** (which
extracts to C, as HACL*/EverCrypt does) is the chosen tool there — see
`proofs/fstar/`. `proofs/rocq/` holds the longer-horizon stretch goal
(algorithm-level proofs, e.g. the policy engine's decision function).

## Run the active tiers
```
tlc tla/BootTransaction.tla -config tla/Bounded.cfg   # tier 1
cargo kani -p storage                                 # tier 2
```

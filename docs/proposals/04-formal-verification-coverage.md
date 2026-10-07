# Proposal: Improve the Formal Verification Coverage

Status: proposal only — no code changed by this document.

## 1. The challenge, restated precisely

> The repository leverages Kani and TLA+ to prove transaction logic, but
> keeping formal proofs updated as features are added is difficult. Expand
> the Kani proof harnesses for edge-case hardware inputs.

## 2. What exists today, and the specific gaps in it (evidence)

Per `docs/adr/0002-tla-and-kani.md`: "TLA+ model-checks the transaction, Kani
proves the parsers panic-free." Both halves are real and passing, but each
has a concrete, evidenced gap:

### 2.1 Kani (`crates/storage/src/proofs.rs`, `spec/kani/PARSERS.md`)

Six harnesses, all in `storage` only — `grep -rn "kani::proof" crates/` finds
no Kani harness outside this one crate. The one that stands out:

```rust
#[kani::proof]
#[kani::unwind(6)]
fn parse_partitions_never_panics() {
    let hbuf: [u8; 96] = kani::any();
    if let Ok(h) = GptHeader::parse(&hbuf) {
        let arr: [u8; 512] = kani::any();
        let _ = parse_partitions(&h, &arr);
    }
}
```

`parse_partitions` (`crates/storage/src/gpt/mod.rs:87-102`) loops
`0..(header.num_entries as usize)`, and `GptHeader::parse` itself enforces
`num_entries <= 512` (`gpt/mod.rs:48`, with the comment "Guard against absurd
counts that could inflate later allocation"). The harness unwinds only **6**
iterations. Kani's unwinding bound caps how many loop iterations are actually
explored; a harness unwound to 6 says nothing about iterations 7 through 512
— exactly the range the code's own documented invariant claims to cover. This
is not a hypothetical nitpick: it is the textbook case loop contracts exist
to fix (see §3.2), and it is the single clearest, most mechanical
"edge-case hardware input" gap the prompt asks to close, because a
maximally-sized malicious/corrupt GPT (`num_entries` anywhere in 7..512) is
exactly the kind of adversarial/corrupt hardware input this parser exists to
survive.

### 2.2 TLA+ (`spec/tla/BootTransaction.tla`) models the abstract machine, not the concrete tie-break

`TLA+`'s `RollBack` action:

```tla
RollBack == /\ phase = "Failed" /\ \E e \in Entries : Bootable(e)
            /\ selected' = IF Bootable(lastGood) THEN lastGood
                          ELSE CHOOSE e \in Entries : Bootable(e)
            /\ phase' = "Selected" /\ UNCHANGED <<triesLeft, lastGood, committed>>
```

`CHOOSE e \in Entries : Bootable(e)` is TLA+'s "pick any one witness" operator
— it proves *some* bootable entry is always available (which is what
`NoDeadEnd` actually needs), but it does **not** encode the Rust
implementation's specific, documented tie-break order. Compare
`crates/transaction/src/lib.rs:166-189` (`pick_fallback`), whose own doc
comment spells out a strict priority: (1) last-good if still bootable, (2)
any OTHER bootable entry **in canonical graph order**, explicitly "rolling to
a different known option rather than repeating a just-failed one," (3) retry
the just-failed entry itself if it still has tries, (4) `None`. The TLA+
spec and the Rust code both happen to satisfy `NoDeadEnd`, but TLC's proof of
the theorem does not actually exercise or validate the Rust priority order —
a bug that picked the WRONG bootable entry (e.g. ignored `last_good`
entirely, or picked in reverse graph order) would still satisfy the existing
TLA+ `RollBack` action and would still pass `./manage.py test`'s example-based
unit tests only by luck of which cases those five tests happen to cover.
There is no proof, of either kind, that the two agree on *which* entry.

### 2.3 The genuinely safety-critical pure crates have zero Kani coverage

`transaction`, `health`, `graph`, `policy`, `discovery`, `config` are all
`#![forbid(unsafe_code)]`, all pure, all host-testable with mocks — exactly
the profile Kani is cheapest and most valuable on — and none of them has a
single `#[kani::proof]`. `Transaction::stage()`'s `saturating_sub` on a
`BTreeMap` entry defaulted via `.or_insert(self.max_tries)`, and
`pick_fallback`'s `last_good`/`failed` `Option<EntryId>` comparisons, are
exactly the "edge-case hardware input" surface (what if `tries_left` already
contains a stale `0` from a previous run? what if `max_tries` was
reconfigured smaller than an already-persisted count? — `Transaction::new`
clamps `max_tries.max(1)` at `lib.rs:71` but nothing clamps an individual
persisted `tries_left` entry against the *new* `max_tries`) the prompt names,
currently covered only by five example-based `#[test]`s
(`transaction/src/lib.rs:229-301`), not exhaustive symbolic coverage.

## 3. Research: the right current tool for each gap

### 3.1 Kani function contracts and loop contracts (fixes §2.1 cheaply, enables §2.3)

The Kani project's own description (Kani: A Model Checker for Rust,
<https://arxiv.org/pdf/2607.01504>; reference docs at
<https://model-checking.github.io/kani/reference/experimental/loop-contracts.html>)
is built for precisely this gap: "bounded model checking fills the automated
end of this gap by encoding default safety properties... and checking them
exhaustively up to a bound" but "bounded analysis alone cannot guarantee
correctness beyond the unwinding depth" — stated as the known limitation
`parse_partitions_never_panics` currently has. The fix is **loop contracts**:

```rust
#[kani::loop_invariant(/* i <= header.num_entries as usize */)]
```

placed on `parse_partitions`'s loop, which "replace[s] exhaustive unrolling
with an inductive argument and yield[s] proofs that do not depend on the
unwinding bound" — i.e. a proof that covers the full documented `0..=512`
range at a fraction of the CBMC solving cost a literal `#[kani::unwind(513)]`
would pay. Function contracts (`#[kani::requires(..)]` /
`#[kani::ensures(|ret| ..)]`, verified via a dedicated
`#[kani::proof_for_contract(f)]` harness) are the matching tool for §2.3:
they let a harness state and check a *specification* (e.g. "`stage()` never
increases `tries_left`; `resolve()` from `Phase::Failed` is always `Ok`")
rather than merely "doesn't panic," which is exactly the stronger claim
§2.2/§2.3 need.

**Stability caveat (verify before depending on this):** the reference page
fetched during this research states both features currently require nightly
feature gates (`#![feature(stmt_expr_attributes)]`,
`#![feature(proc_macro_hygiene)]`) and CLI flags (`-Z function-contracts`,
`-Z loop-contracts`) — i.e., experimental/unstable as of this writing.
**Confirm current stability against the exact `cargo-kani` version this
repo's toolchain resolves to before relying on them** (CLAUDE.md §5 evidence
discipline applies to tooling, not just the `uefi` crate). If still
unstable, §4.1 below (raising the unwind bound) is the fallback that needs no
unstable features at all.

### 3.2 Refinement discipline between TLA+ and Kani/Rust

No tooling automatically keeps a TLA+ action and a Kani-proven Rust function
in lockstep — ADR 0002 names the two tiers but, per its own text, does not
prescribe a refinement check between them. The practical discipline used by
projects that run both (e.g. AWS's own use of TLA+ for protocol design
alongside Kani for the Rust implementation on projects like `s2n-quic`, which
is the precedent this repository's ADR 0002 is itself clearly modeled on) is:
the TLA+ action and the Rust function it governs must make the **identical**
claim, and a Kani harness over the Rust function is what stands in for "TLC
checked the abstract version" at the concrete-code level. Where they
currently diverge (§2.2), that discipline is not yet being followed for the
fallback-order logic specifically, even though it is followed for the phase
machine as a whole.

## 4. Proposed expansion plan (ordered, each step separately landable)

### 4.1 Close the existing unwind gap in `storage` (cheapest, do first)

In `crates/storage/src/proofs.rs`, either:
- raise `#[kani::unwind(6)]` to a bound that actually covers the documented
  512-entry cap (note: CBMC's exhaustive-unwinding cost grows with the bound,
  so measure `cargo kani -p storage` wall-clock time before and after), or
- once §3.1's stability caveat is resolved, replace it with a
  `#[kani::loop_invariant]` proof instead, which covers the full range
  without paying per-iteration unwinding cost.

Either way, state the chosen bound and the reason in a code comment, per
CLAUDE.md §0.2's "state the reason for each change" rule.

### 4.2 Add a new `crates/transaction/src/proofs.rs` (mirrors `storage`'s pattern exactly — one file, one job)

Three concrete harnesses, each closing a specific gap named in §2.3/§2.2:

```rust
// (sketch — final predicates to be written against the real Transaction API)
#[kani::proof]
fn stage_never_panics_and_only_decreases_tries() { /* symbolic max_tries, tries_left */ }

#[kani::proof]
fn resolve_from_failed_always_succeeds() {
    // the executable, Rust-level form of the TLA+ NoDeadEnd theorem:
    // for any symbolic graph of up to N entries (keep N small, e.g. 3-4,
    // to keep the state space tractable) and any symbolic tries_left/health,
    // Transaction::resolve() from Phase::Failed is never Err(_).
}

#[kani::proof]
fn pick_fallback_matches_documented_priority_order() {
    // proves the specific claim lib.rs's own doc comment makes:
    // last_good (if bootable) is chosen over any other candidate.
}
```

`resolve_from_failed_always_succeeds` is the direct Rust-level proof of the
same property `spec/tla/BootTransaction.tla`'s `THEOREM Spec => []NoDeadEnd`
proves abstractly — this is the concrete link ADR 0002 names as the point of
having both tiers, made real rather than aspirational.

### 4.3 Update `BootTransaction.tla`'s `RollBack` action to encode the real priority order

Replace the under-specified `CHOOSE e \in Entries : Bootable(e)` branch with
an expression mirroring `pick_fallback`'s actual order (last-good → other
bootable in canonical order → retry-same), so the TLA+ model and the
Kani-proven Rust function make the *same* specific claim, not two
independently-true-but-different ones. This is a small, one-action edit to
`spec/tla/BootTransaction.tla`, directly evidenced by (and required to track)
`crates/transaction/src/lib.rs:173-189`.

### 4.4 Model the firmware-watchdog liveness case (ties directly to Proposal 03)

Add a `TimedOut` action to `BootTransaction.tla`, enabled from `Launched`
exactly like `DetectFailure`, representing "the firmware watchdog fired with
no cooperative signal from the OS" (Proposal 03 §4.2). Without this, the
spec's own `EventuallyResolved` liveness theorem implicitly assumes
`DetectFailure` is always eventually reachable — true only once Proposal 03's
watchdog change lands; until then the spec is proving a claim the real
system (pre-watchdog) does not actually guarantee for a silent hang.

### 4.5 Process fix for "difficult to keep updated" (the prompt's actual root complaint)

Add an explicit checklist to `spec/README.md`: any change to
`crates/transaction/src/lib.rs`'s transition functions must, in the same
commit, update the matching `spec/tla/BootTransaction.tla` action **and**
the matching `crates/transaction/src/proofs.rs` harness. This is a
review-time convention, not new tooling — flagging it as a documented rule
closes the gap between "possible to keep in sync" and "actually enforced,"
without taking on a new CI-tooling project that is out of scope here.

## 5. Explicitly out of scope

- `spec/proofs/rocq/` and `spec/proofs/fstar/` are both already marked, in
  their own README, as optional/future "Tier 3" work, "NOT on the critical
  path." Nothing in the prompt's wording ("the repository leverages Kani and
  TLA+") asks for these, and expanding them is a materially larger
  undertaking (deductive proof, not model checking) — noted here as a future
  option, not part of this proposal.

## Sources

- Kani: A Model Checker for Rust (architecture, function/loop contracts): <https://arxiv.org/pdf/2607.01504>
- Kani loop contracts reference: <https://model-checking.github.io/kani/reference/experimental/loop-contracts.html>
- Kani function contracts community discussion: <https://redlib.hackliberty.org/r/KaniRustVerifier/comments/1ae479f/function_contracts_for_kani>

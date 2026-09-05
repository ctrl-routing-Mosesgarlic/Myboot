# F* — verified signature verification (near-term, report §3.13)

Goal: prove the Secure Boot signature-check routine is memory-safe and
functionally correct, then extract it to C (via KaRaMeL) and link it into
`crates/security/src/signature`. This mirrors the HACL*/EverCrypt approach.

Placeholder: `SecureBootVerify.fst`. Toolchain (F*, KaRaMeL) to be added when
this tier is activated.

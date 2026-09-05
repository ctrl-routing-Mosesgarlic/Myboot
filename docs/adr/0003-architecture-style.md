# ADR 0003 — Architecture style: Hexagonal core + Microkernel plug-ins, SRP/SSOT

Status: accepted. Context: reconciling the enterprise-architecture deep-dive with
MyBoot's pre-OS, no_std, single-address-space reality.

## Decision
MyBoot is a **modular monolith** (one UEFI binary, one address space) organised as a
**Hexagonal (Ports & Adapters)** core with a **Microkernel plug-in** provider model.

- **Domain core (inner ring):** `graph` (the Boot Graph) + the transaction/policy rules.
  It has zero knowledge of uefi-rs, filesystems, or partition formats.
- **Driven ports (outbound):** `providers::BootProvider` (OS adapters plug in here),
  and `persistence` (NVRAM/ESP stores). The core defines the interface; adapters implement it.
- **Driving ports (inbound):** the three interfaces — `ui-gfx`, `ui-shell`, `host-cli` —
  invoke engine use-cases; they contain no boot logic.
- **Adapters (outer ring):** `platform` (uefi-rs), `storage` (on-disk formats),
  `arch` (assembly). Nothing in the outer ring is referenced by the core.
- **Dependency Rule:** all source dependencies point inward toward `graph`. Enforced by
  the Cargo dependency graph (a crate cannot depend on a crate that would create a cycle).

## Principles (per the deep-dive's modularity metrics)
- **SRP** (Single Responsibility Principle): each *module* has one cohesive
  responsibility. This does NOT mean one class per tiny operation — we optimise for
  **high cohesion, low coupling**, not an explosion of microscopic files.
- **SSOT** (Single Source of Truth): each authoritative fact has exactly one home.
  Examples: the **Boot Graph** is the sole model of what is bootable; `spec/tla/
  BootTransaction.tla` is the sole source of truth for the transaction's state shape
  (the Rust `Phase` enum mirrors it); `esp/EFI/MyBoot/config.toml` is the sole
  declarative desired-state; firmware NVRAM is the sole authority for actual boot order.

## Explicitly rejected (from the deep-dive, as inapplicable here)
Microservices, SOA/ESB, event brokers, serverless, and any relational database —
all presuppose a network, an OS, and multiple processes that do not exist before boot.
EDA's "event" idea survives only as the in-memory boot-result protocol, not a broker.

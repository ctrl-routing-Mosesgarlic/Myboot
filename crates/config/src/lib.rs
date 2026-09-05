//! config — MyBoot's declarative configuration and firmware reconciliation
//! (report §3.8, Figure 3.4; ADR 0003). SSOT for the *desired* boot state.
//!
//! Three cohesive responsibilities, one module each (SRP):
//!   * `toml`      — a small, panic-free TOML-subset reader (no external crate).
//!   * `schema`    — the typed configuration model + validation.
//!   * `reconcile` — diff desired-vs-actual firmware state and produce a
//!                   reversible repair plan (with a snapshot for rollback).
//!
//! Pure and firmware-independent: it plans against the `VarStore` port, so the
//! whole thing is host-tested with a mock. The actual `SetVariable` happens in
//! the `platform` adapter when the engine executes the plan.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

pub mod toml;
pub mod schema;
pub mod reconcile;

#[cfg(test)]
extern crate std;

pub use schema::{Config, ConfigError, EntrySpec, PolicyKind, RollbackKind};
pub use reconcile::{ReconcilePlan, PlanStep, Snapshot};

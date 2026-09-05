# MyBoot — Implementation Blueprint

> **Purpose of this document.** This is the single source of truth for implementing
> MyBoot. It is written so that any contributor — human or AI — can pick up any file
> and know exactly what it must do, what it may assume, what it must never do, and how
> it connects to everything else, **without re-deriving context and without
> hallucinating**. When in doubt, this document and the six-chapter report in
> `docs/report/` govern. Section references like "§3.7" point to that report.

---

## 0. North star (read this first, every time)

MyBoot is an **intelligent, transactional, cross-OS UEFI boot-environment manager**
written in Rust. The **contribution is the management layer**, not the UI or the
language:

- a semantic **Boot Graph** (disks → OS → generations/kernels/specialisations/recovery),
- an **OS-adapter** architecture (`BootProvider`),
- a **deterministic, explainable policy engine** (no ML in the boot path),
- a **transactional** boot lifecycle with cooperative success-confirmation and rollback,
- a **declarative** configuration reconciled against firmware (repairs boot-order drift),
- three coordinated interfaces over one engine (graphical menu, pre-boot shell, host CLI),
- **verified** critical parts: the boot transaction in TLA+, the parsers in Kani.

We do **not** compete on "prettier menu / auto-discovery / boot Linux" — those exist.
We compete on **management**: health, transactions, confirmation, rollback, recovery,
reconciliation. (§2, §6.)

---

## 1. Hard rules (invariants — never violate)

1. **`no_std` everywhere in the engine and the UEFI binary.** `std` does not exist
   before an OS. Use `core` + `alloc` (the allocator is installed by
   `uefi::helpers::init()`). Only `host-cli` and `xtask` are `std`.
2. **Parsers are total and panic-free.** Every parser is a function over `&[u8]` that
   returns `Result<_, _>` and **never** indexes a slice directly — always `slice.get(..)`.
   These are the functions Kani proves (§3.12, §5.4). A parser that can panic is a bug.
3. **Delegate, don't replace.** MyBoot launches/federates native components
   (chainload Windows Boot Manager, Linux EFI stub, resolve NixOS generation). It does
   **not** rewrite any OS's own boot configuration. (§2.7)
4. **The policy engine is deterministic and explainable.** No neural nets, no
   randomness, no "AI" in the boot decision path. ML is allowed only for offline
   diagnostics/log analysis, never for choosing what to boot. (§2.7 "Don't call it AI".)
5. **A boot is a transaction.** Stage (charge the attempt) → launch → confirm on the
   next boot → commit or roll back. An **unconfirmed boot is treated as a failure**. The
   machine must **never dead-end**: from a failed boot, either a fallback exists or
   recovery is entered. (§3.7, §3.11.)
6. **No relational database.** Persistence = firmware NVRAM variables (tiny state) +
   ESP files (config + growing log) + in-memory Boot Graph (derived, never persisted).
   (§3.9.)
7. **Honesty of scope.** This is design + specification + partial verification. Do not
   present emulated/expected results as measured hardware results. Do not invent
   empirical numbers (timings, counts). (§5.1, §6.5.)
8. **Dependency direction is one-way.** Interfaces and engine crates depend inward on
   the model (`graph`) and the platform wrapper (`platform`) — never the reverse. No
   crate above the model layer may touch a filesystem/partition/OS directly. (§3.4.)

---

## 2. Architecture & dependency direction (§3.4, Figure 4.1)

```
                 bin/myboot  (BOOTX64.EFI)
                        │  calls engine::run()
                        ▼
   ui-gfx / ui-shell ──► engine ◄── host-cli (separate std binary, offline)
                        │
   policy · transaction · config · health · discovery · providers · security
                        │
                    graph (Boot Graph — the uniform model)
                        │
        storage · persistence · platform (uefi-rs)   +   arch (asm)
                        │
                 UEFI firmware (BootOrder/BootNext/BootCurrent, GOP, storage, vars)
```

All arrows point down/inward. `graph` is the hub every layer speaks in terms of.

---

## 3. Per-file responsibilities

Legend for **Status**: `stub` = documented placeholder, `partial` = real
types/skeleton present, `spec` = specification/proof artefact. Every `.rs` file
begins with a `//!` doc line naming its report section.

### 3.1 `bin/myboot/` — the UEFI application
| File | Function | Expected contents | Status |
|---|---|---|---|
| `Cargo.toml` | Binary manifest; depends on `engine` + `platform`; builds `BOOTX64.EFI`. | — | done |
| `src/main.rs` | Entry point (§4.4). `#![no_main] #![no_std]`, `#[entry] fn efi_main() -> Status`, `uefi::helpers::init()`, acquire image FS + GOP, call `engine::run()`. On `Ok` the return type is `Infallible` (handoff never returns); on `Err`, fall back to the shell / return `LOAD_ERROR`. | partial |

### 3.2 `crates/platform/` — safe uefi-rs wrappers (§3.4, §4.4–4.5)
No crate above `platform` calls uefi-rs directly.
| File | Function | Expected contents | Status |
|---|---|---|---|
| `src/lib.rs` | Re-exports the four submodules. | — | partial |
| `src/gop/mod.rs` | GOP acquisition + double-buffered blt presentation (§4.5). `get_handle_for_protocol::<GraphicsOutput>()` → `open_protocol_exclusive` → `current_mode_info().resolution()`; `blt(BltOp::BufferToVideo{ .. BltRegion::Full | SubRectangle })`. | stub |
| `src/fs/mod.rs` | Read files from the ESP via Simple File System (§4.4). | stub |
| `src/vars/mod.rs` | `GetVariable`/`SetVariable` for global vars (BootOrder, Boot####, BootNext, BootCurrent) **and** MyBoot vendor-GUID vars (§3.7, §3.9). | stub |
| `src/block/mod.rs` | Raw sector reads via Block I/O, feeding the storage parsers (§4.6). | stub |

### 3.3 `crates/storage/` — parsers over untrusted bytes (§3.12, §4.6, §5.4)
**Every function here is total and panic-free (Kani-verified).**
| File | Function | Expected contents | Status |
|---|---|---|---|
| `src/lib.rs` | Re-exports parsers; `#[cfg(kani)] mod proofs`. | — | partial |
| `src/gpt/mod.rs` | GPT header/entry parsing. `GptHeader::parse(&[u8]) -> Result<_,GptError>`; checks `b"EFI PART"` at offset 0; helpers use `slice.get(..)`. **Reference implementation already written — copy its discipline everywhere.** | partial |
| `src/mbr/mod.rs` | Protective/legacy MBR parsing. | stub |
| `src/fat/mod.rs` | FAT reader for the ESP. | stub |
| `src/pe/mod.rs` | PE/COFF recognition of EFI images (for chainloading). | stub |
| `src/tests/mod.rs` | Off-metal `proptest` cases for all parsers (§5.4). | stub |
| `src/proofs.rs` *(create)* | The `#[kani::proof]` harnesses (§3.12). See `spec/kani/PARSERS.md`. | to create |

### 3.4 `crates/discovery/` — enumerate the world (§3.3, §4.7)
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Assemble a `VolumeSet`; orchestrate the four sources. | partial |
| `src/esp/mod.rs` | Locate + read the EFI System Partition. | stub |
| `src/variables/mod.rs` | Read Boot####, BootOrder, BootNext, BootCurrent. | stub |
| `src/bls/mod.rs` | Boot Loader Specification entries. | stub |
| `src/bootspec/mod.rs` | NixOS bootspec generation discovery (RFC 0125). | stub |

### 3.5 `crates/graph/` — the Boot Graph (§3.5, Figure 3.2) — **the hub**
| File | Function | Expected contents | Status |
|---|---|---|---|
| `src/lib.rs` | The data model. `BootGraph → DiskNode → OsNode → BootEntry`; enums `OsKind`, `EntryRole`, `BootMethod`, `Health`. `EntryId` is **stable across reboots** (derive deterministically from OS+role+loader path, NOT discovery order). Health is first-class. Derived state — never persisted. | partial |

### 3.6 `crates/providers/` — OS adapters (§3.6)
Implements `trait BootProvider { kind; discover; validate; describe; boot }`.
`boot()` returns `Result<Infallible, BootError>` — **on success it never returns** (handoff).
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | The `BootProvider` trait + shared `VolumeSet/BootTarget/Descriptor/…` types. | partial |
| `src/windows/mod.rs` | Chainload `\EFI\Microsoft\Boot\bootmgfw.efi` (**not** `bootmgr.efi`). | stub |
| `src/linux/mod.rs` | Two levels: EFI-stub chainload; and direct load (kernel+initrd → uses `arch` handoff). | stub |
| `src/nixos/mod.rs` | Resolve generations/specialisations from bootspec. | stub |
| `src/generic/mod.rs` | Chainload any other native `.efi`. | stub |
| `src/recovery/mod.rs` | Surface recovery environments. | stub |

### 3.7 `crates/health/` · `crates/policy/`
| File | Function | Status |
|---|---|---|
| `health/src/lib.rs` | Validate entries (files present, signatures ok); compute `Health` with a machine-readable reason (§3.5). | stub |
| `policy/src/lib.rs` | Deterministic default selection from health + last-good history + config. Must be **explainable**: every recommendation has a stated reason. No ML. (§2.7, §3.3.) | stub |

### 3.8 `crates/transaction/` — the boot transaction (§3.7, §3.11)
Mirrors `spec/tla/BootTransaction.tla` **exactly**. Phases: Idle→Selected→Validated→Staged→Launched→{Confirmed | Failed}→{RollBack→Selected | Recovery}.
| File | Function | Expected contents | Status |
|---|---|---|---|
| `src/lib.rs` | The `Phase` enum + the transaction driver. **Charge the attempt at Stage (decrement the counter) BEFORE handoff**, so a crash counts against the entry. | partial |
| `src/marker/mod.rs` | Cooperative success marker via firmware vars, following the **systemd Boot Loader Interface** (see §5 facts). Read on next boot to commit or fail. | stub |
| `src/rollback/mod.rs` | On Failed: pick a bootable fallback (prefer last-good) → back to Selected; if none bootable → Recovery. **Never dead-end.** | stub |

### 3.9 `crates/config/` — declarative config + reconciliation (§3.8, Figure 3.4)
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Wire schema + reconcile. | partial |
| `src/schema/mod.rs` | Typed model of `config.toml` (serde). Fields per `esp/EFI/MyBoot/config.toml`. | stub |
| `src/reconcile/mod.rs` | `plan_entries(config)` → desired Boot#### + BootOrder; `read_firmware()`; diff; apply atomically with a retained **snapshot**; verify; on mismatch restore. Repairs boot-order drift (e.g. Windows re-assertion). | stub |

### 3.10 `crates/persistence/` — no database (§3.9)
Three tiers, mapped exactly:
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Re-export the three stores. | partial |
| `src/nvram/mod.rs` | Tiny must-persist state in firmware variables (default/selected/one-shot/attempt counters) under MyBoot's vendor GUID. Keep small — NVRAM is scarce & wears. | stub |
| `src/espfile/mod.rs` | Read/write `/EFI/MyBoot/config.toml` on the ESP. | stub |
| `src/log/mod.rs` | Append-only boot-history log (NDJSON/CBOR) under `/EFI/MyBoot/log/`. Growing data goes here, **never** to NVRAM. | stub |

### 3.11 `crates/ui-gfx/` — graphical menu (§3.10.1, Figures 3.5–3.6; pipeline §4.5)
Thin face over the Boot Graph. Contains no boot logic.
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Compose the menu app. | partial |
| `src/render/mod.rs` | Off-screen back buffer (`Vec<BltPixel>`) + dirty-rect blit to GOP (§4.5). | stub |
| `src/widgets/mod.rs` | Boot Graph menu, cards, health badges, details panel (matches Figure 3.5). | stub |
| `src/input/mod.rs` | Unify keyboard + Simple Pointer / USB HID events. | stub |
| `src/theme/mod.rs` | Palette + typography (see `assets/`). | stub |

### 3.12 `crates/ui-shell/` — pre-boot shell (§3.10.2, Figure 3.7)
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Wire the REPL. | partial |
| `src/commands/mod.rs` | `disks · os list · inspect · health · verify · boot · default · diagnostics · log · reboot · firmware`. Operates over the Boot Graph. | stub |

### 3.13 `crates/security/` — Secure Boot + integrity (§3.13)
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Wire the two modules. | partial |
| `src/secureboot/mod.rs` | Trust-chain state; prefer deferring image verification to firmware `LoadImage` (Lanzaboote pattern). | stub |
| `src/signature/mod.rs` | Explicit signature checks for directly-loaded images. **This is the one place `spec/proofs/fstar` (verified crypto) may be linked in.** | stub |

### 3.14 `crates/arch/` — the ONLY assembly (§4.4) — **see §4 below for scope**
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | Gate on `target_arch`; document what asm is/isn't for. | partial |
| `src/x86_64/mod.rs` | Pull in `handoff.s` via `global_asm!`; export `cpu`, `handoff`. | partial |
| `src/x86_64/cpu.rs` | Inline-asm wrappers (`cli`, `hlt`, `read_cr3`, …) via `core::arch::asm!`. | partial |
| `src/x86_64/handoff.rs` | Rust side of the direct-kernel handoff; `extern "C"` to the trampoline. | partial |
| `src/x86_64/handoff.s` | **Hand-written assembly.** The jump-to-kernel trampoline for LinuxDirect, after ExitBootServices. Never returns. | partial |

### 3.15 `crates/engine/` — orchestration (§3.3)
| File | Function | Status |
|---|---|---|
| `src/lib.rs` | `run()` executes the 9-stage pipeline (§3.3): discover → model → validate → decide → present → transact → execute → confirm. Returns `Result<Infallible, EngineError>`. Depends on all engine crates. | partial |

### 3.16 `host-cli/` — host-side management tool (§3.10.3) — **std binary, offline**
| File | Function | Status |
|---|---|---|
| `Cargo.toml` | `std`; built for the host target, excluded from the UEFI workspace. | done |
| `src/main.rs` | CLI entry; dispatch. | partial |
| `src/commands/mod.rs` | `install · sign · config apply/show · reconcile [--repair] · status · gc · enroll-keys · log export`. Reaches firmware vars via efivarfs/efibootmgr (Linux) or firmware APIs (Windows). | stub |

### 3.17 `xtask/` — automation (§4.8–4.9)
| File | Function | Status |
|---|---|---|
| `src/main.rs` | Subcommands: `build`, `run` (QEMU+OVMF), `kani`, `tlc`, `package-esp`, `test`. | stub |

### 3.18 `spec/` — formal artefacts (see `spec/README.md`)
| Path | Function | Status |
|---|---|---|
| `tla/BootTransaction.tla` | **Tier 1 (active).** The boot-transaction model; `NoDeadEnd` (safety) + `EventuallyResolved` (liveness). | spec |
| `tla/Bounded.cfg` | TLC config: `Entries={a,b,c}`, `MaxTries=2`. | spec |
| `kani/PARSERS.md` | **Tier 2 (active).** Describes the harnesses in `crates/storage/src/proofs.rs`. | spec |
| `proofs/fstar/SecureBootVerify.fst` | **Tier 3 (optional, near-term).** Verified signature check → extract to C → link into `security`. | placeholder |
| `proofs/rocq/BootPolicy.v` | **Tier 3 (stretch).** Deductive proof the policy function is total + explainable. | placeholder |

### 3.19 Other
| Path | Function |
|---|---|
| `esp/EFI/MyBoot/config.toml` | The declarative config, in its real on-disk location (§3.8). |
| `esp/EFI/BOOT/` | Where the build copies `BOOTX64.EFI` (firmware fallback path). |
| `test/fault-injection/matrix.toml` | The fault matrix (§5.6) as machine-readable cases. |
| `test/integration/README.md` | Integration scenarios (§5.5). |
| the `./manage.py smoke` command · `./manage.py verify` | Run under QEMU+OVMF; run all verification. |
| `.github/workflows/ci.yml` | fmt + clippy + build + `cargo kani -p storage`. |
| `docs/report/*.docx` | The six-chapter report (authoritative design). |
| `docs/adr/*` | Architecture Decision Records. |
| `docs/diagrams/*.svg` | Source SVGs for every report figure. |

---

## 4. Assembly — exactly what it is and is NOT for

**Assembly lives only in `crates/arch/src/x86_64/handoff.s`** (+ inline `asm!` in
`cpu.rs`). Scope, precisely:

- **Needed for:** (1) the **direct-kernel handoff** on the LinuxDirect path — set the
  stack/registers per the boot protocol and `jmp` to the kernel entry point **after**
  `ExitBootServices`, never returning; (2) a few **privileged CPU ops** (`cli`, `hlt`,
  control-register reads) not exposed safely by `core`.
- **NOT needed for:** chainloading Windows / Linux-EFI-stub / generic `.efi` (those use
  firmware `LoadImage`/`StartImage`), GOP drawing, discovery, parsing, config, or the
  UI. Those are 100% Rust.
- **Rule:** keep `arch` tiny. If you're writing assembly anywhere else, stop — it almost
  certainly belongs in Rust. On `x86_64-unknown-uefi` the firmware already put us in
  long mode; we are not writing a stage-1 loader.

---

## 5. Verified facts (use these; do NOT re-derive or guess)

These were confirmed against primary sources during design. An implementer must treat
them as ground truth to avoid hallucination.

**UEFI boot variables (global-variable GUID `8BE4DF61-93CA-11D2-AA0D-00E098032B8C`):**
- `Boot####` = an `EFI_LOAD_OPTION` (Attributes `UINT32`, FilePathListLength `UINT16`,
  Description `CHAR16*`, FilePathList, OptionalData).
- `BootOrder` = array of `UINT16`. `BootNext` = single `UINT16`, tried once next boot,
  **deleted by firmware before handoff** (loop prevention). `BootCurrent` = the option
  chosen this boot.
- Accessed via `GetVariable`/`SetVariable` runtime services.

**systemd Boot Loader Interface (vendor GUID `4a67b082-0a4c-41cf-b6c7-440b29bb8c4f`):**
- `LoaderEntrySelected` (loader→OS: which entry booted), `LoaderEntryDefault` /
  `LoaderEntryOneShot` (OS→loader: default), `LoaderBootCountPath` (filename encodes
  remaining/used attempt counters). `systemd-bless-boot` marks a boot good on
  `boot-complete.target`. This is the model for MyBoot's success marker (§3.7).

**On-disk signatures:** GPT header signature is ASCII `"EFI PART"` at LBA 1; PE images
start `MZ`/`PE\0\0`; the ESP is FAT32.

**Windows loader path:** `\EFI\Microsoft\Boot\bootmgfw.efi` (**never** `bootmgr.efi`).

**uefi-rs (current API — the global model; the old `SystemTable<Boot>` parameter is GONE):**
- `#![no_main] #![no_std]`; `#[entry] fn efi_main() -> Status`; `uefi::helpers::init()`.
- `uefi::boot` / `uefi::system` freestanding functions; image FS via
  `boot::get_image_file_system(boot::image_handle())`.
- GOP: `boot::get_handle_for_protocol::<GraphicsOutput>()` → `open_protocol_exclusive`
  → `current_mode_info().resolution()` → `blt(BltOp::BufferToVideo{ .. })` with
  `BltRegion::Full` or `SubRectangle`; `frame_buffer()` for direct access.

**Target & tooling:** `x86_64-unknown-uefi` (Tier-2; **no** `-Z build-std`). Fallback
boot path `\EFI\BOOT\BOOTX64.EFI`. QEMU+OVMF: `-drive if=pflash …OVMF_CODE.fd` (ro) +
`OVMF_VARS.fd` (rw) + `-drive format=raw,file=fat:rw:esp` + `-serial stdio` + `-s -S`
(GDB on tcp:1234).

**Verification tooling:** Kani = AWS bounded model checker for Rust (proved Firecracker's
parser panic-free). TLA+/TLC = Lamport's spec + model checker (AWS since 2011). F* =
proof-oriented, extracts to C (HACL*/EverCrypt, deployed in Firefox/Linux/WireGuard).

---

## 6. Cross-cutting contracts (get these exactly right)

**`BootProvider` (§3.6).** `boot()` returns `Result<Infallible, BootError>`: success =
handoff = never returns; it yields a value **only on failure**. A provider knows nothing
of other providers.

**Transaction ↔ TLA+ (§3.7, §3.11).** The Rust `Phase` enum and the transitions in
`transaction/` must correspond 1:1 to `spec/tla/BootTransaction.tla`. If you change one,
change the other and re-run TLC. `Stage` decrements the attempt counter **before**
handoff.

**Persistence tiers (§3.9).** Tiny + must-persist + must-survive-to-OS → NVRAM. Config →
ESP TOML. Growing/log → ESP files. Derived (the Boot Graph) → memory only.

**Reconciliation (§3.8).** `plan → read → diff → (snapshot) apply → verify → repaired |
restore`. Reversible. Never edits an OS's own store.

---

## 7. Build / run / verify

```bash
rustup target add x86_64-unknown-uefi
cargo build --workspace --target x86_64-unknown-uefi     # engine + UEFI binary
./manage.py smoke                                     # boot under emulated UEFI
cargo kani -p storage                                     # tier-2 parser proofs
tlc spec/tla/BootTransaction.tla -config spec/tla/Bounded.cfg   # tier-1 model check
cargo build -p myboot-host --target x86_64-unknown-linux-gnu    # host CLI (std)
```

---

## 8. Implementation order (milestones)

1. **M1 — Boot & draw.** `platform::gop` + `ui-gfx::render` + a hardcoded menu that
   boots one chainloaded entry under QEMU. Proves the toolchain end-to-end.
2. **M2 — Parsers (verified).** Finish `storage::{gpt,mbr,fat,pe}` + `src/proofs.rs`;
   `cargo kani -p storage` green. This is the safety foundation.
3. **M3 — Discovery + Boot Graph.** `discovery::*` → `graph`; render the real graph.
4. **M4 — Providers.** Windows chainload, Linux EFI-stub, NixOS generations. (LinuxDirect
   + `arch` handoff can come later.)
5. **M5 — Transaction + marker.** Implement `transaction` to match the TLA+ spec; wire
   the success marker; demonstrate rollback in QEMU.
6. **M6 — Config + reconcile + host CLI.** Declarative config; drift repair; `host-cli`
   install/sign/reconcile.
7. **M7 — Shell, recovery, diagnostics, security.** Fill `ui-shell`, recovery view,
   Secure Boot.
8. **M8 — Fault-injection harness.** Implement `test/fault-injection` against QEMU
   snapshots (§5.6).
9. **(Optional) M9 — Tier-3 proofs.** Activate `spec/proofs/fstar` for the signature
   path; explore `rocq` for policy properties.

---

## 9. Do-NOT list (common ways to break context)

- ❌ Don't use `std` in the engine or UEFI binary. ❌ Don't index slices in parsers.
- ❌ Don't write `bootmgr.efi` (it's `bootmgfw.efi`). ❌ Don't pass `SystemTable<Boot>` to
  `#[entry]` (old API). ❌ Don't add `-Z build-std` (target is Tier-2).
- ❌ Don't put ML/heuristics you can't explain in the boot decision. ❌ Don't add a
  database. ❌ Don't let a failed boot dead-end. ❌ Don't edit an OS's own boot store.
- ❌ Don't invent empirical results/timings. ❌ Don't write assembly outside `crates/arch`.
- ❌ Don't claim deductive proof where we use model checking; be precise: TLA+ = model
  checking, Kani = bounded model checking, F*/Rocq = deductive (optional/future).

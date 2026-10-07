# Proposal: Standardize the Transactional Callback Interface

Status: proposal only — no code changed by this document.

## 1. The challenge, restated precisely

> The transactional rollback system is only as good as the operating system's
> ability to say "I successfully booted up." If an OS crashes halfway through
> loading user space, it might fail to clear the flag.

## 2. What exists today (evidence)

MyBoot's transaction model is a faithful mirror of `spec/tla/BootTransaction.tla`
(confirmed line-for-line against `crates/transaction/src/lib.rs`'s own doc
comment, lines 1-14). The relevant half for this challenge:

- **Charge-before-handoff** (`crates/transaction/src/lib.rs:107-116`,
  `stage()`): the attempt is decremented and the `BootAttempt` marker is
  armed (`crates/transaction/src/marker.rs:47-50`, called from
  `pipeline.rs:154`) **before** control leaves MyBoot. This already correctly
  handles "the OS never gets anywhere at all" — the attempt is burned
  regardless.
- **Confirm is entirely cooperative and Linux-only.** The only code that ever
  flips `confirmed` to `true` is `host-cli/src/commands/bless.rs`, which:
  requires `efivarfs` to be mounted (`is_available()`), requires root, and
  must be invoked by *something inside the booted OS* — per its own doc
  comment, "mirrors `systemd-bless-boot good`." Nothing in this repository
  ever calls it automatically; it is a manual/unit-triggered Linux command.
  `grep -rn -i "bless" crates/ host-cli/` finds no Windows- or
  macOS-equivalent anywhere.
- **Reconciliation assumes a marker gets read back at all.**
  `pipeline.rs:176-188` (`reconcile_previous_boot`) only runs if MyBoot itself
  regains control on a subsequent boot and finds a marker. If the OS *hangs*
  (kernel panic with no reboot, a wedged early-userspace that never panics or
  reboots, a silent freeze) rather than cleanly rebooting, firmware never
  hands control back to MyBoot at all — the machine just sits there. This is
  a real, distinct gap from "the OS crashed and rebooted without blessing,"
  which the current design already handles correctly (unconfirmed → treated
  as failed → rollback next boot, per `marker.rs`'s own module doc).

So there are two separable problems hiding in the one sentence the prompt
gives:

1. **Coverage gap**: only Linux (via a manual/unit-triggered CLI command) can
   ever confirm a boot. Windows entries (`crates/providers/src/windows.rs`)
   have no confirming counterpart at all — every Windows boot looks
   "unconfirmed" forever from MyBoot's point of view, which would eventually
   exhaust `tries_left` and force a spurious rollback/recovery even though
   Windows boots fine.
2. **Liveness gap**: a true hang (not a crash-and-reboot) never reaches
   `DetectFailure`/`resolve()` at all, because firmware never regains control.
   `transaction::Transaction::has_way_forward` (`lib.rs:191-205`) and the
   TLA+ `EventuallyResolved` theorem both implicitly assume the machine
   *eventually gets back to MyBoot* — true for a crash, not true for a wedge.

## 3. Prior art — how other transactional boot-counting systems close these gaps

### 3.1 The wire-format/protocol itself is already the right shape

- **UAPI Group Boot Loader Specification, "Boot Counting"**
  (<https://uapi-group.org/specifications/specs/boot_loader_specification/>):
  encodes a *tries-left* and *tries-done* pair per entry, decremented on each
  boot attempt, and removed entirely on success ("entry state becomes
  'good'"). MyBoot's `Marker { entry, confirmed }` (`marker.rs:22-25`) plus
  `Transaction.tries_left: BTreeMap<EntryId, u8>` is a generalization of the
  same idea, moved off the filename and into one vendor NVRAM variable keyed
  by the portable `EntryId` rather than by OS-specific path — this is
  **already OS-agnostic at the storage layer**. `ports::VarStore` has zero
  Linux-specific knowledge. The gap is entirely about *who calls in* on each
  platform, not the protocol.
- **systemd's Automatic Boot Assessment**
  (<https://systemd.io/AUTOMATIC_BOOT_ASSESSMENT>) names the exact same
  shape: a `LoaderBootCountPath` EFI variable the boot loader sets, and
  `systemd-bless-boot.service` — gated on reaching `boot-complete.target` —
  clears the counter. MyBoot's `host-cli bless` is this pattern already;
  what's missing is the equivalent *trigger* on non-systemd platforms.

### 3.2 Windows already has its own cooperative signal — reuse it, don't replace it

Microsoft's BCD exposes `bootstatuspolicy`
(e.g. `bcdedit /set {current} bootstatuspolicy IgnoreAllFailures` /
`DisplayAllFailures`), which gates whether the **Windows Boot Manager itself**
hands control to WinRE after repeated failures. This is Windows' own internal
boot-health tracking and is not something MyBoot can read or drive
pre-boot — it operates entirely within `bootmgfw.efi`'s own territory, after
MyBoot has already chainloaded it. It is not a substitute for a MyBoot-side
signal, but it confirms Windows already has a notion of "did this boot
complete," which a user-mode confirm agent can key off of (reaching a stable
logon = the natural Windows analogue of `boot-complete.target`).

### 3.3 Embedded bootloaders already solve the "silent hang" liveness gap: the hardware watchdog

U-Boot's Boot Count Limit
(<https://docs.u-boot-project.org/en/latest/api/bootcount.html>) is the
general embedded answer: `bootcount` is persisted and incremented on **every**
boot attempt (U-Boot's own moral equivalent of MyBoot's charge-before-handoff
rule), and it is "the responsibility of some application code... to reset the
variable bootcount to 0 when the system booted successfully" — i.e. exactly
MyBoot's `confirmed` flag. Critically, embedded designs pair this with a
**hardware watchdog timer**, not software-only cooperation: if the OS never
gets far enough to pet the watchdog, the watchdog itself forces a reset,
which is what actually guarantees the bootloader regains control to act on
the (still-unconfirmed) counter. UEFI firmware exposes this exact mechanism
as a boot service — `EFI_BOOT_SERVICES.SetWatchdogTimer` — which MyBoot does
not currently arm anywhere (`grep -rn "watchdog\|Watchdog" crates/` finds no
hits).

## 4. Proposed design

### 4.1 Close the Windows coverage gap — a small, separate confirm agent, not a protocol change

The `marker`/`VarStore` contract (`crates/transaction/src/marker.rs`) needs
**no changes** — it is already OS-agnostic. What's missing is a Windows-side
caller equivalent to `host-cli bless`:

1. A small, new, separate artifact — out of scope for the `no_std` Rust
   workspace, analogous in spirit to the existing `install.ps1` at the repo
   root — that performs the Windows equivalent of
   `host-cli/src/commands/bless.rs::run()`: read the current `BootAttempt`
   vendor variable and rewrite it with `confirmed = true`, using the Win32
   firmware-variable APIs Windows exposes to privileged processes
   (`GetFirmwareEnvironmentVariableEx`/`SetFirmwareEnvironmentVariableEx`,
   the same family `efibootmgr`-equivalent Windows tools use) against the
   same vendor GUID `host-cli/src/efivars.rs` already defines.
2. Trigger it on "Windows reached a stable, logged-in desktop" — the natural
   analogue of systemd's `boot-complete.target` — via a Scheduled Task
   (Task Scheduler "At log on" trigger, optionally with an additional delay
   to avoid blessing a desktop that locks up moments after login), installed
   by the existing `install.ps1` alongside its current firmware-entry setup,
   rather than inventing a new installation mechanism.
3. This is additive and keeps the pure `transaction`/`marker` crates
   completely unchanged — it is exactly as "standardized" as the prompt asks
   for, because the standard (the marker format) already exists; what was
   missing was a second implementer.

### 4.2 Close the silent-hang liveness gap — arm the UEFI watchdog before handoff

1. In `crates/engine/src/pipeline.rs`, immediately before `launch()` is
   called (`pipeline.rs:156-166`, right after `tx.launch()`), call
   `boot::set_watchdog_timer` (verify the exact uefi-0.33 signature against
   docs.rs before coding, per CLAUDE.md §5) to arm a watchdog sized generously
   enough that a healthy OS reaching early userspace has time to clear/reset
   it (several minutes, not seconds — needs empirical tuning per supported
   OS, since Linux/systemd and Windows both already reset or disable the UEFI
   watchdog as a normal part of boot in most configurations, which needs
   **verification** rather than assumption before relying on it as the sole
   safety net).
2. On a genuine hang (OS never reaches the point where it would reset the
   watchdog), firmware's own watchdog fires a reset — which hands control
   back to MyBoot with the marker still unconfirmed, flowing through the
   **already-correct** `reconcile_previous_boot` → rollback path with **zero
   changes to the pure `transaction` crate**. This turns "the OS failed to
   say it's fine" and "the OS can't say anything because it's wedged" into
   the same, already-handled `Failed → resolve()` transition.
3. This change belongs entirely in `crates/platform`/`crates/engine`
   (firmware-facing, already-permitted unsafe territory per CLAUDE.md §0.7)
   and must be proven in QEMU before being trusted: the ISO/smoke test
   procedure (root `CLAUDE.md` §3.3/3.4) should gain a new assertion that
   deliberately hangs the guest (e.g. boot a kernel with `init=/bin/sleep` or
   similar) and confirms MyBoot's **next** boot shows the entry as having
   consumed an attempt and, once attempts are exhausted, rolls back — this is
   a new, concrete, falsifiable test the existing harness does not have
   today.

### 4.3 What NOT to change

- Do not touch the `Marker` wire format, `Transaction`'s phase machine, or
  `pick_fallback`'s priority order — none of that is OS-specific, and the
  evidence above shows the gap is entirely about *callers*, not the
  *protocol*. Re-encoding the marker would be scope creep CLAUDE.md §0.3
  explicitly forbids ("change only what the evidence requires").

## Sources

- UAPI Group Boot Loader Specification, boot-counting section: <https://uapi-group.org/specifications/specs/boot_loader_specification/>
- systemd Automatic Boot Assessment: <https://systemd.io/AUTOMATIC_BOOT_ASSESSMENT>
- `systemd-bless-boot.service(8)`: <https://man.archlinux.org/man/systemd-bless-boot.service.8.en>
- Windows BCD `bootstatuspolicy` / automatic repair: Microsoft Learn troubleshooting guide (<https://learn.microsoft.com/en-us/troubleshoot/azure/virtual-machines/windows/troubleshoot-guide-windows-boot-manager-menu>)
- U-Boot Boot Count Limit: <https://docs.u-boot-project.org/en/latest/api/bootcount.html>

# MyBoot — How It Works, Step by Step

This document walks through everything MyBoot does, from the moment you press the
power button to the moment your chosen operating system takes over. Each step is
explained twice: first in **plain language** (the "what and why"), then in
**technical language** (the "how", with the actual mechanisms and UEFI details).

MyBoot is a **UEFI boot manager**: a small program whose whole job is to find the
operating systems on your computer, show you a menu, and hand control to the one
you pick — safely, and without ever leaving you stuck at a black screen.

---

## The 30-second version

When your computer turns on, the firmware runs MyBoot. MyBoot looks at every disk,
figures out which operating systems are installed, shows you a menu, and starts the
one you choose. If that one fails to start, it automatically tries the next healthy
option instead of giving up. It remembers whether the last boot succeeded, so it can
avoid an option that's broken. That's it — it's the "traffic controller" that sits
between the firmware and your operating systems.

---

## Step 0 — Power on: the firmware runs MyBoot

**Plain language.** When you press power, a chip on your motherboard (the UEFI
firmware) wakes up first. Its job is tiny: initialise the hardware and then start
*one* program from a special disk area. On a machine where MyBoot is installed, that
program is MyBoot. Think of the firmware as the building's front desk that hands the
visitor off to the receptionist (MyBoot), who actually knows where everyone is.

**Technical language.** The platform firmware completes its DXE/BDS phases and, per
its BootOrder, loads a PE32+ EFI application from the EFI System Partition (ESP) — a
small FAT32 partition. MyBoot is that application (`\EFI\MyBoot\BOOTX64.EFI`, or the
fallback `\EFI\BOOT\BOOTX64.EFI`). Firmware calls its `efi_main` entry point with an
image handle and a pointer to the UEFI System Table. MyBoot is built `#![no_std]`,
`#![no_main]` for the `x86_64-unknown-uefi` target; the `#[entry]` macro wires up
`efi_main`.

---

## Step 1 — Wake up: allocator, logging, graphics

**Plain language.** Before MyBoot can do anything useful it has to switch on a few of
its own faculties: the ability to use memory, the ability to print diagnostic
messages (so we can see what it's doing), and the ability to draw on the screen.

**Technical language.** `uefi::helpers::init()` installs the panic handler and logger.
The binary enables the uefi crate's `global_allocator` feature, which designates a
`#[global_allocator]` backed by the firmware's pool allocator (`AllocatePool`), so
Rust heap types (`Vec`, `String`, `Box`) work. MyBoot sets the log level to `Info` so
its pipeline diagnostics appear on the serial console. Graphics are acquired lazily
later, through the Graphics Output Protocol (GOP).

---

## Step 2 — Look everywhere: scan every disk (multi-volume)

**Plain language.** This is one of the most important steps and a recent upgrade. A
real computer often has **several disks**, and operating systems can live on any of
them — Windows on one, Linux on another, a rescue system on a USB stick, an installer
on a DVD or ISO. MyBoot now looks at **every** storage device the firmware can see,
not just the one it was started from. This is exactly what well-known managers like
rEFInd do, and it's what lets MyBoot handle "any number of operating systems" across
"any number of disks."

**Technical language.** MyBoot enumerates every handle that supports the
`SimpleFileSystem` protocol via `boot::locate_handle_buffer(ByProtocol(SFS_GUID))`.
Each handle becomes a `Volume` (a `FileStore` bound to that filesystem). A
`MultiVolume` wrapper then presents all of them as a single `FileStore`: a read
resolves on the first volume that has the path, `exists` is true if **any** volume
has it, and `list_dir` returns the de-duplicated union across volumes. Because the
rest of the pipeline talks to the `FileStore` interface, none of the discovery or
health code had to change — it now transparently sees the whole machine.

---

## Step 3 — Figure out what's installed: discovery

**Plain language.** Now MyBoot reads those disks and works out *what operating systems
are actually there*. It looks in the standard places each OS puts its startup files —
the Windows Boot Manager, the folders Linux distributions use, the boot entries the
firmware itself records, the special files NixOS creates for each of its versions
("generations"). Every real bootable thing it finds becomes one **entry**.

**Technical language.** Discovery is pure orchestration over `storage` parsers and the
`ports` I/O abstractions, so it's fully unit-tested with mocks. Independent source
modules each turn one kind of evidence into a `RawEntry`:

- `variables` — existing UEFI `Boot####` NVRAM options (like `efibootmgr`).
- `esp` — well-known loaders on the ESP, e.g. `\EFI\Microsoft\Boot\bootmgfw.efi`
  (Windows) and the `\EFI\BOOT\BOOTX64.EFI` fallback (like rEFInd's scan).
- `distros` — vendor directories under `\EFI\` (ubuntu, fedora, arch, nixos, …),
  preferring `shimx64.efi` for Secure Boot, reading `os-release` for a pretty name.
- `bls` — Boot Loader Specification Type #1 entries (like systemd-boot).
- `bootspec` — NixOS bootspec generations (RFC 0125, like Lanzaboote).

`lib.rs` merges the raw entries, de-duplicates by a **stable id**, and `build_graph`
groups them into a **Boot Graph**. A crucial rule: entries with neither a loader nor a
kernel (e.g. a firmware `Boot####` that points at a whole device, not a file) are
**dropped** — nothing can launch them, so they must not sit in the graph.

---

## Step 4 — Check what's healthy: health assessment

**Plain language.** Not every entry MyBoot finds is actually bootable right now — a
file might be missing, for instance. So MyBoot checks each entry and marks it healthy
or not. Only healthy entries are offered as real choices.

**Technical language.** `health::assess_all` walks the graph and, for each entry with
a loader path, verifies the loader file exists (through the `MultiVolume` store, so a
loader on any disk counts). Entries are marked `Healthy`, `Degraded`, or `Unbootable`.
Health feeds both the menu ordering and the policy decision.

---

## Step 5 — Learn from last time: reconcile the previous boot

**Plain language.** MyBoot remembers what it tried last time. Before it does anything
new, it checks: did the operating system we launched last time actually come up
successfully? If yes, it records that choice as "known good." If it never confirmed
(it crashed or hung), MyBoot counts that as a failed attempt so it can avoid a broken
option in future. This is what stops a bad update from trapping you in a boot loop.

**Technical language.** MyBoot uses a **transactional boot** model. Before handoff it
"arms" a marker in NVRAM; the OS (or a small confirm step) is expected to "bless" the
boot, clearing the marker. On the next boot, `reconcile_previous_boot` reads the
pending marker: a *confirmed* marker records the entry as `last_good`; an *unconfirmed*
marker means the previous attempt failed, and the try-count already charged at Stage
lets policy avoid an exhausted entry. Then the marker is cleared.

---

## Step 6 — Read your preferences: configuration

**Plain language.** MyBoot reads its settings file — how long to wait before booting
the default, which OS is the default, whether to ask for confirmation, and so on. If
there's no settings file, it uses sensible defaults.

**Technical language.** `load_config` reads `\EFI\MyBoot\config.toml` (best-effort:
falls back to defaults if absent or malformed). It provides the default entry id,
`timeout_secs`, policy strategy, `max_tries`, and rollback behaviour. Parsing is total
and panic-free.

---

## Step 7 — Decide the default: policy

**Plain language.** MyBoot works out which entry *should* boot if you don't touch the
keyboard. The logic is deterministic and explainable — never a black box: it honours a
one-shot "boot this once" request if present, otherwise your configured default if
it's healthy, otherwise the last-known-good option, otherwise simply the healthiest
one. Crucially, it can always tell you *why* it picked what it picked.

**Technical language.** `policy::decide(graph, history, strategy)` returns a `Decision
{ chosen, reason }` where `reason` is a closed enum (`OneShot`, `ConfiguredDefault`,
`LastGood`, `Healthiest`, …) so the choice is auditable and localisable. There is no
machine learning in the boot path — the decision is a pure function of the graph,
history, and strategy.

---

## Step 8 — Show the menu: presentation

**Plain language.** MyBoot draws the boot menu on screen, highlights the default, and
counts down. You can press the arrow keys to pick a different OS, Enter to boot it, or
just wait for the countdown to boot the default. If the machine has no usable graphics
for some reason, MyBoot skips the menu and boots the default so you're never stuck.

**Technical language.** `present` acquires a `Framebuffer` via GOP and renders a menu
built from the graph's bootable entries, polling the `Input` protocol for arrow/Enter/
Escape within the timeout. On timeout it returns the policy's chosen id; if GOP is
unavailable it logs that and returns the decision headlessly. The return value is the
`EntryId` to boot.

---

## Step 9 — Commit carefully: the transaction

**Plain language.** Once an OS is chosen, MyBoot doesn't just leap. It records "I'm
about to try this one" so that if it doesn't come back, next time it knows this attempt
failed. This bookkeeping is what makes MyBoot safe rather than reckless.

**Technical language.** A `Transaction` runs `select` → `validate` → `stage`. `select`
refuses an entry that is unbootable or out of tries. `stage` charges an attempt and
returns a `ChargeAttempt` effect; MyBoot persists the try state and **arms the NVRAM
marker before handoff**. The state machine mirrors a TLA+ model of the same
`tries_left` / `last_good` semantics, so its safety properties are checked formally.

---

## Step 10 — Hand off: start the operating system

**Plain language.** This is the moment MyBoot gets out of the way and starts your OS.
There are two ways it does this:

- **Chainloading** (the normal case): MyBoot tells the firmware to start the OS's own
  startup program — the Windows Boot Manager, GRUB, systemd-boot, shim, or an
  installer's loader. The key detail (a recent, important upgrade): MyBoot points the
  firmware at the loader **on its own disk**, so that loader can find *its* own files
  (Windows's boot database, GRUB's config, the ISO's kernel). Loading it any other way
  would leave it blind and unable to start.
- **Direct kernel boot** (fallback): for a Linux kernel that isn't a self-contained
  EFI program, MyBoot can set up the boot information itself and jump straight into the
  kernel.

**Technical language.** `launch` dispatches on the `LaunchPlan`:

- `Chainload`: `MultiVolume::chainload` finds the volume holding the loader, builds a
  full device path (**the volume's device path + a `FilePath` node**), and calls
  `boot::load_image(FromDevicePath{…})` then `start_image`. Loading via device path
  (not a memory buffer) is what installs the loaded image's `DeviceHandle`, so
  chainloaded managers can locate their own configuration and kernels — the UEFI-spec
  `FileDevicePath(DeviceHandle, FilePath)` idiom. A `FromBuffer` path remains as a
  fallback for self-contained images. Secure Boot verification is preserved either way
  (the firmware runs the Security2 protocol on the image).
- `DirectKernel`: `direct_boot` builds the Linux boot parameters and E820 map from the
  UEFI memory map, calls `exit_boot_services`, and jumps via the `arch` trampoline
  (a real 64-bit boot-protocol handoff in x86-64 assembly). This is the fallback for
  non-EFI-stub kernels; the default Linux path is EFI-stub chainload, which is
  Secure-Boot-verified.

---

## Step 11 — Never dead-end: automatic fallback

**Plain language.** What if the option you (or the countdown) picked fails to start?
Older boot managers would just error out and dump you back to the firmware. MyBoot
doesn't. If the first choice can't start, it automatically tries the next healthy
option, and the next, until something boots — and only if *every* option fails does it
return to the firmware. You should never be left staring at a dead screen because one
entry was broken.

**Technical language.** Stages 8–9 build an ordered candidate list — the chosen entry
first, then the remaining bootable entries healthiest-first (`policy::ranked_
candidates`) — and try each through the transaction + handoff. `launch` only returns on
*failure* (success never returns), so a returned error moves to the next candidate.
`plan_for` is checked **before** an attempt is charged, so an unplannable entry doesn't
waste a try. Only when the whole list is exhausted does `run` return
`NoBootableEntries` for the caller to drop to firmware/recovery.

---

## Step 12 — The next boot: confirm or roll back

**Plain language.** If the OS came up fine, great — MyBoot will remember it as a good
choice. If it didn't (crash, hang, half-finished update), MyBoot counts that against
the entry, and after a few failures it will stop offering the broken one and fall back
to the last version that worked. This is the "safety net" that makes risky updates
survivable.

**Technical language.** On the subsequent boot, Step 5's reconcile reads the armed
marker: unconfirmed ⇒ the attempt is treated as failed, and because Stage already
decremented `tries_left`, policy will avoid the entry once its tries are exhausted and
prefer `last_good`. Confirmed ⇒ the entry becomes the new `last_good`. This closes the
transactional loop: **arm before handoff, reconcile on next boot**.

---

## The shape of the whole thing (architecture)

**Plain language.** MyBoot is built like an onion. The centre contains the pure
"thinking" parts — deciding what's bootable, which to pick, how to stay safe — and none
of that code touches the hardware directly. Only the outer layer knows about the actual
firmware. This is why almost all of MyBoot can be tested on a normal computer without a
real boot at all, and why it's reliable: the tricky logic is isolated, total, and
formally checked.

**Technical language.** Hexagonal (ports-and-adapters) architecture. The domain core
(`graph`, `policy`, `health`, `transaction`, `config`, `discovery`, `storage`, …) is
`#![no_std]`, `#![forbid(unsafe_code)]`, and depends only on the `ports` traits
(`VarStore`, `FileStore`). The `platform` crate is the single UEFI adapter implementing
those ports against firmware services (plus `arch` for the assembly handoff). Unsafe
code is confined to `platform`, `arch`, and `direct_boot`. Parsers are total and
panic-free; critical properties are checked with Kani and a TLA+ model. The result is
~105 host tests over the pure crates, and a firmware binary whose risky surface area is
small and auditable.

---

## Quick glossary

- **UEFI / firmware** — the low-level software on your motherboard that starts things.
- **ESP** — EFI System Partition, the FAT32 area where boot programs live.
- **Boot manager vs boot loader** — a *manager* (MyBoot, rEFInd, systemd-boot) shows a
  menu and hands off; a *loader* (GRUB, Windows Boot Manager) actually loads a kernel.
- **Chainload** — starting another boot program from within a boot manager.
- **Device path** — UEFI's way of naming exactly where a file lives (which disk,
  which partition, which path), so firmware can load it correctly.
- **Boot Graph** — MyBoot's internal, unified list of everything bootable.
- **Marker / transaction** — the record MyBoot arms before booting so it can tell,
  next time, whether the boot succeeded — enabling automatic rollback.

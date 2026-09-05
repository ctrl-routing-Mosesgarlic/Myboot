# MyBoot — Testing & Installation Guide

MyBoot is an intelligent, transactional, cross-OS **UEFI boot-environment
manager** written in Rust (`no_std`) with one small piece of x86-64 assembly.
This guide takes you from "the source compiles" to "it runs, it's tested, and
it's installed on a real machine" — in that order, because installing a
bootloader you haven't tested is how machines stop booting.

> **Golden rule (from the release-certification checklist below):** never treat
> "the binary launches in QEMU" as production readiness, and never install onto
> your primary machine first. QEMU → disposable disk/VM → secondary hardware →
> primary machine.

---

## 0. What you need

| Tool | Why | Install |
|------|-----|---------|
| Rust (stable) + `x86_64-unknown-uefi` target | build the loader | `rustup target add x86_64-unknown-uefi` |
| QEMU | run the VM | your distro's `qemu-system-x86` |
| OVMF | UEFI firmware for QEMU | your distro's `ovmf` / `edk2-ovmf` |
| `efibootmgr` | register the firmware entry (real HW) | your distro's package |

If your distro's OVMF is missing or old, use the daily prebuilt images from
`rust-osdev/ovmf-prebuilt` (linked in §6).

---

## 1. How the system works (the 30-second model)

```
UEFI firmware
     │  (runs \EFI\MyBoot\BOOTX64.EFI)
     ▼
  MyBoot engine ── pipeline ──────────────────────────────────────────┐
     │   1. read the boot-attempt marker (was the last boot confirmed?)│
     │   2. DISCOVER: scan NVRAM Boot####, the ESP, BLS entries,        │
     │      NixOS bootspec, and \EFI\<vendor>\ distro dirs              │
     │   3. build the BOOT GRAPH  (disks → OS → entries)               │
     │   4. HEALTH-check each entry (loader/kernel present, etc.)      │
     │   5. POLICY decides the default — deterministically, with a     │
     │      machine-readable reason (no ML in the boot path)           │
     │   6. PRESENT the menu (GOP graphics; timeout; keyboard)         │
     │   7. TRANSACTION: select → validate → stage (charge an attempt) │
     │      → arm the marker → launch                                   │
     ▼
  LAUNCH:
     • Chainload (default): firmware LoadImage/StartImage of the OS's
       own EFI loader — Windows Boot Manager, shim/GRUB, or a Linux
       EFI-stub kernel. Secure Boot stays intact (firmware verifies).
     • DirectKernel (fallback, non-EFI-stub kernels): build boot_params
       + E820 from the UEFI memory map, ExitBootServices, and jump via
       the arch assembly trampoline.
     ▼
  Selected OS boots. On the next boot MyBoot reads the marker: if the
  OS never confirmed success, policy can roll back to last-good.
```

Everything above the launch line is pure Rust and unit-tested on your host. The
launch line and the graphics/NVRAM glue need real firmware (QEMU or hardware).

---

## 2. Step 1 — Run the host test suite (no firmware needed)

The pure crates (`no_std`, firmware-free) test natively on your machine.

```sh
./manage.py test
```

Expected: **104 tests pass across 14 crates** (graph, storage, ports,
persistence, config, policy, health, discovery, transaction, security,
providers, ui-shell, ui-gfx, arch) with **0 failures**. This covers the parsers,
the Boot Graph, the policy engine, the transaction state machine, discovery
(including distro detection and EFI-stub-vs-direct classification), and the
boot-protocol data structures.

If a crate fails to build, you're on a toolchain older than the code expects;
update Rust and retry.

---

## 3. Step 2 — Run it in QEMU with a synthetic multi-OS ESP (safe)

This builds the loader, stages a **fake** ESP containing MyBoot plus stand-in
Windows/Ubuntu/NixOS loaders, and boots it under OVMF — so you can watch
discovery, the menu, health tags, and the timeout without any real disk.

```sh
./manage.py smoke
# firmware not in /usr/share/OVMF? point at it:
OVMF_CODE=/path/OVMF_CODE.fd OVMF_VARS=/path/OVMF_VARS.fd ./manage.py smoke
```

What to look for on the serial console / GOP screen:

- MyBoot lists **three** systems (Ubuntu from `\EFI\ubuntu`, Windows from
  `\EFI\Microsoft\Boot`, and its own entry), proving discovery works.
- "Ubuntu 24.04 LTS" appears — that exact string is read from the staged
  `os-release`, proving `PRETTY_NAME` parsing.
- The default is chosen with a stated reason; the countdown runs; arrow keys +
  Enter select.

The script copies `OVMF_VARS.fd` to a writable temp so firmware NVRAM works,
exactly as the Rust UEFI Book recommends (§6).

> The staged loaders are not real OSes, so *selecting* one won't boot Linux —
> this stage validates discovery, the graph, health, policy, and the UI. To test
> an actual hand-off, use a disposable VM with a real OS installed (§5).

---

## 4. Step 3 — The release-certification gate (from the uploaded checklist)

Before any hardware install, walk the **GO / NO-GO** matrix. This is the
checklist you provided, mapped to MyBoot's current state. Copy it into the repo
as `CERTIFICATION.md` and tick boxes as you verify them on real firmware.

**Already demonstrated (host tests + QEMU synthetic ESP):**

- [x] Clean build · `cargo test` (104) · pure-crate coverage
- [x] Linux discovery (BLS + `\EFI\<vendor>\` distro dirs)
- [x] Windows discovery (chainload `bootmgfw.efi`)
- [x] NixOS generation discovery (bootspec)
- [x] Multiple OSes in one graph; broken/stale entries skipped, not panicked
- [x] Corrupt/missing config → recovery (config crate tests)
- [x] Deterministic, explainable default selection (policy tests)
- [x] Transactional boot: attempt charged before hand-off; last-good recorded on
      confirm; never dead-ends (transaction tests)
- [x] Install / uninstall path that **coexists** and never formats the ESP

**Must be verified on real firmware before you claim them** (the checklist is
right that QEMU passing ≠ hardware passing):

- [ ] UEFI boot on Dell / HP / Lenovo / ASUS / a desktop board
- [ ] GPT and (if claimed) MBR; FAT32 ESP
- [ ] Secure Boot **ON** with a signed binary + invalid-signature rejection
- [ ] NVRAM: create entry → reboot → persists; change order; firmware reset
- [ ] Windows Update → reboot → MyBoot still works, Windows still works
- [ ] NixOS: new generation → reboot → **fails** → boot previous → recovers
- [ ] Power-loss during install / config write / update
- [ ] Multiple disks (NVMe + SATA + USB); missing disk; missing OS
- [ ] UI fallback path if GPU/GOP init fails
- [ ] Unsafe-code audit (every `unsafe` in `arch`, `platform`, `engine`)

**Wording the checklist recommends adopting:** don't claim "supports any OS."
Claim: *"supports operating systems and boot protocols for which MyBoot
implements a verified loader/discovery adapter; unknown bootable EFI applications
may be chainloaded."* That is exactly MyBoot's architecture (OS-adapter
providers + generic EFI chainload), and it's a stronger, honest claim.

---

## 5. Step 4 — Destructive testing in disposable VMs (before hardware)

Build a matrix of throwaway VMs and test the real hand-off and failure paths.
Automate as much as possible. Suggested VMs: Windows-only; Ubuntu-only;
Windows+Ubuntu; Windows+NixOS; Ubuntu+Fedora+NixOS; 8-OS; 100 synthetic entries;
corrupt config; corrupt filesystem; Secure Boot. For each: install MyBoot,
register it, reboot, confirm discovery, then boot each entry and confirm it
reaches userspace. Only after this passes do you touch physical hardware, and
even then a **secondary** machine or a spare disk first.

---

## 6. How to test an already-built, similar system (reference)

MyBoot's QEMU recipe is modeled on the maintained Rust UEFI tooling — use these
as the authoritative reference for the firmware-facing test setup:

- **Rust UEFI Book — "Running in a VM"** (the canonical QEMU + OVMF + VVFAT
  recipe MyBoot's QEMU harness follows):
  <https://rust-osdev.github.io/uefi-rs/tutorial/vm.html>
- **uefi-rs test runner** (`cargo xtask run` builds a UEFI app and runs it in
  QEMU — the same pattern as MyBoot's `xtask`):
  <https://github.com/rust-osdev/uefi-rs/tree/main/uefi-test-runner>
- **Prebuilt OVMF images** (if your distro's are missing/old):
  <https://github.com/rust-osdev/ovmf-prebuilt>
- **rust-osdev `bootloader`** (a mature Rust BIOS/UEFI bootloader with a QEMU
  test harness — study its BIOS/UEFI separation):
  <https://github.com/rust-osdev/bootloader>

For the firmware contract itself, treat the **UEFI Specification** and the UEFI
Forum's test tools as authoritative over any blog post.

---

## 7. Installing on real hardware (command line)

MyBoot's equivalent of `grub-install /dev/sdX && update-grub` is **`myboot
install`** plus one `efibootmgr` registration. The installer is deliberately
conservative: it writes only to `<ESP>/EFI/MyBoot`, autodetects the ESP, and
**never** formats or overwrites another OS's loader.

### 7a. One-liner (curl | sh) — the packaged path

```sh
# download + install (does NOT touch NVRAM unless you add --register)
curl -fsSL https://raw.githubusercontent.com/OWNER/myboot/main/scripts/install.sh | sudo sh

# force the ESP mount point (e.g. systemd-boot layouts mount it at /boot)
curl -fsSL .../manage.py install | sudo sh -s -- --esp /boot

# build from source instead of downloading a release
curl -fsSL .../manage.py install | sudo sh -s -- --from-source

# also register the firmware entry in one shot
curl -fsSL .../manage.py install | sudo sh -s -- --register --disk /dev/nvme0n1 --part 1
```

The `sh -s -- <args>` form is the standard way to pass flags to a piped
installer (the same mechanism rustup and others use). Set `MYBOOT_REPO=you/myboot`
to point the download at your fork. The script refuses to run on non-UEFI
systems, refuses to run on NixOS (see §8), locates the ESP, and delegates to the
host CLI — which coexists with whatever is already on the ESP.

### 7b. Manual path (what the one-liner does, step by step)

```sh
# 1. build the loader for UEFI
cargo build -p myboot --release --target x86_64-unknown-uefi

# 2. build the host CLI
cargo build --manifest-path host-cli/Cargo.toml --release \
    --target "$(rustc -vV | sed -n 's/host: //p')"

# 3. install onto the ESP (autodetects /boot/efi, /boot, or /efi) + create config
sudo ./host-cli/target/*/release/myboot install \
    --efi target/x86_64-unknown-uefi/release/myboot.efi
#   → creates <ESP>/EFI/MyBoot/config.toml   (this is where the config comes from)
#   → copies the loader to <ESP>/EFI/MyBoot/BOOTX64.EFI
#   → takes <ESP>/EFI/BOOT/BOOTX64.EFI only if nothing else owns it

# 4. register with firmware (the install command prints this line for your disk)
sudo efibootmgr --create --disk /dev/nvme0n1 --part 1 \
    --loader '\EFI\MyBoot\BOOTX64.EFI' --label 'MyBoot' --unicode
```

**Where does `config.toml` come from?** `myboot install` creates it (and never
overwrites an existing one). That was previously undocumented; it is now a
first-class command.

### 7c. Uninstall

```sh
curl -fsSL https://raw.githubusercontent.com/OWNER/myboot/main/scripts/uninstall.sh | sudo sh
# or:  sudo ./host-cli/target/*/release/myboot uninstall
```

Removes only `<ESP>/EFI/MyBoot` and prints how to delete the NVRAM entry. Every
other OS stays bootable through its own loader.

---

## 8. Special note: NixOS + systemd-boot (this applies to your current machine)

Your `nixos-rebuild switch` output tells the whole story:

```
Skipping "/boot/EFI/systemd/systemd-bootx64.efi", same boot loader version in place already.
Skipping "/boot/EFI/BOOT/BOOTX64.EFI", same boot loader version in place already.
```

Three facts follow, and MyBoot is built to respect all three:

1. **The ESP is mounted at `/boot`, not `/boot/efi`.** MyBoot's installer now
   autodetects this (`/boot/efi` → `/boot` → `/efi`), so `myboot install` finds
   the right partition with no flag. You can still force it with `--esp /boot`.

2. **systemd-boot owns `/boot/EFI/BOOT/BOOTX64.EFI`** (the removable-media
   fallback). MyBoot **must not** clobber it — and doesn't: the installer takes
   the fallback path only when nothing else owns it, and here it's owned, so
   MyBoot installs to `/boot/EFI/MyBoot/BOOTX64.EFI` and leaves systemd-boot's
   fallback untouched. (Verified: after `myboot install` the pre-existing
   `EFI/BOOT/BOOTX64.EFI` is still present.)

3. **On NixOS you should NOT `curl | sh` a bootloader.** NixOS manages the
   bootloader declaratively; the very next `nixos-rebuild switch` will reconcile
   the ESP back to what your configuration says and undo an imperative install —
   exactly the "same boot loader version in place already / activating the
   configuration" reconciliation you saw. The install script therefore **refuses
   to run on NixOS** and points here.

   The correct NixOS approach is declarative: disable the built-in loader and
   register MyBoot as an external one, e.g.

   ```nix
   # configuration.nix (sketch — adapt paths to your build output)
   boot.loader.systemd-boot.enable = false;
   boot.loader.grub.enable = false;
   boot.loader.external = {
     enable = true;
     installHook = "${myboot-installer}/bin/myboot-install";  # your packaged installer
   };
   boot.loader.efi.canTouchEfiVariables = true;
   ```

   Then `sudo nixos-rebuild switch` installs MyBoot the NixOS way, and rebuilds
   won't fight it. (Package `myboot install` into `myboot-install` as the hook.)
   This is also the cleanest way to exercise MyBoot's NixOS generation discovery:
   `nixos-rebuild boot`, reboot, and confirm each generation appears under one
   "NixOS" node in the graph.

   MyBoot's own design mirrors this philosophy: it keeps a **desired boot
   configuration** and reconciles the firmware toward it — the same idea that
   makes your systemd-boot install idempotent ("same version in place already").

---

## 9. Release artifacts to publish (so "what am I installing?" has an answer)

For each tagged release, attach: `myboot.efi` (the UEFI loader), `myboot` (the
host CLI), `scripts/install.sh` + `scripts/uninstall.sh`, SHA-256 checksums, and
(if you claim Secure Boot) detached signatures. The one-liner downloads
`myboot.efi` and `myboot` from `releases/latest/download`, so those two asset
names must stay stable.

---

## 10. Where to read more in this repo

- `MyBoot_STRUCTURE.txt` — the crate/module map (which file does what).
- `MyBoot_IMPLEMENTATION_STATUS.md` — what's implemented + the reference trail.
- `MyBoot_BUILD_AND_TROUBLESHOOT.md` — build details and uefi-rs version-sensitive
  spots (E820 / `exit_boot_services` / `allocate_pages`).
- `MyBoot_BLUEPRINT.md` — the architecture and the research thesis.
- `pyboot/` + `manage.py` — the Python tooling (build/test/smoke/iso-test/install/uninstall/usb/verify);
  `verify.sh` (Kani proofs + TLA+ model check).

---

## 11. NixOS: from dev shell to a live system (your machine)

You now have a working `flake.nix` devShell (Rust + `x86_64-unknown-uefi` target,
QEMU, OVMF, and the disk tools). A matching `flake.nix` + `.envrc` now ship in the
repo, so `direnv allow` gives contributors the same environment. Inside the shell,
`OVMF_CODE`/`OVMF_VARS` are exported — which is exactly what the `./manage.py smoke` command
consumes, so it works on NixOS with no extra flags.

### 11.1 Do this FIRST: compile the firmware crates

Everything proven so far (104 tests) is the **pure** crates. The three
firmware-facing crates — `platform`, `engine`, `bin/myboot` — have never been
compiled against the real `x86_64-unknown-uefi` target in development. Your dev
shell is the first place that can. So the very first step is not "install," it's
"build and fix what the real target surfaces":

```sh
cargo build -p myboot --release --target x86_64-unknown-uefi
```

Expect the version-sensitive spots flagged in `MyBoot_BUILD_AND_TROUBLESHOOT.md`
to be where any errors land — the `uefi-rs` calls in `engine/direct_boot.rs`
(`exit_boot_services`, `allocate_pages`, memory-map iteration) and the GOP/SFS
glue in `platform`. If the pinned `uefi = "0.33"` doesn't match your resolved
version, adjust the calls per that guide. **Bring me the first error and we fix
it together** — this is the real "does it work on the target" gate, and it comes
before any hardware.

Confirm the artifact is a UEFI binary, not ELF:

```sh
file target/x86_64-unknown-uefi/release/myboot.efi   # → PE32+ executable (EFI application)
```

### 11.2 The testing ladder (do NOT skip rungs)

1. **Host logic** — `./manage.py test` → 104 pass. ✅ done.
2. **Builds for UEFI** — §11.1. ← you are here.
3. **QEMU, synthetic ESP** — `./manage.py smoke`. Watch discovery + menu.
4. **QEMU, real OS disk** — attach a real Linux/Windows qcow2 as a second drive
   and confirm MyBoot discovers and *chainloads* it. This is the first real
   hand-off test.
5. **USB on real hardware** — §11.3. Non-destructive; this is your first "live
   system" test.
6. **Internal disk** — §11.4. Only after 1–5 pass, and with a rescue USB ready.

### 11.3 Live hardware, the SAFE way — boot from USB (do this first)

This runs MyBoot on real firmware on your actual laptop **without touching your
NixOS install, its ESP, or NVRAM**. If MyBoot misbehaves, you pull the USB and
reboot normally — there is nothing to undo.

```sh
lsblk                         # identify your USB stick, e.g. /dev/sdb
sudo ./manage.py usb --device /dev/sdb
```

`make-usb.sh` refuses any device that isn't marked removable (so you can't hit
your system disk), shows you `lsblk`, and makes you type the device name to
confirm. It writes MyBoot to `\EFI\BOOT\BOOTX64.EFI` on the stick. Then reboot,
open the firmware **one-time boot menu** (usually F12/F9/Esc), and pick the USB.
MyBoot will discover the OSes on your internal disks and present its menu. Boot
each one; confirm each reaches its desktop. **This exercises the real
discovery + chainload path with zero risk.**

### 11.4 Installing onto the internal disk on NixOS (last, and reversibly)

Two facts about your machine (from your `nixos-rebuild` output): the ESP is at
`/boot`, and systemd-boot manages it declaratively. So install MyBoot as an
**additional, non-default** boot entry — never as the replacement — and let
systemd-boot stay the default until MyBoot has earned trust.

**Coexistence install (recommended):**

```sh
# 1. build both binaries (host CLI on the host target)
cargo build -p myboot --release --target x86_64-unknown-uefi
cargo build --manifest-path host-cli/Cargo.toml --release \
    --target "$(rustc -vV | sed -n 's/host: //p')"

# 2. put MyBoot in its own vendor dir on the ESP (nixos-rebuild never touches
#    \EFI\MyBoot, only \EFI\systemd and \EFI\BOOT — so this survives rebuilds)
sudo ./host-cli/target/*/release/myboot install --esp /boot \
    --efi target/x86_64-unknown-uefi/release/myboot.efi
#    → creates /boot/EFI/MyBoot/{BOOTX64.EFI, config.toml}
#    → leaves systemd-boot's \EFI\BOOT\BOOTX64.EFI and \EFI\systemd untouched

# 3. add a firmware entry but DO NOT make it first. Find your ESP disk/part:
lsblk -o NAME,PARTTYPENAME,MOUNTPOINT | grep -i efi        # e.g. nvme0n1 part 1
sudo efibootmgr --create --disk /dev/nvme0n1 --part 1 \
    --loader '\EFI\MyBoot\BOOTX64.EFI' --label 'MyBoot' --unicode
# systemd-boot stays BootOrder[0]; MyBoot is just another entry you can pick.
```

Now you boot normally into systemd-boot every time; to *try* MyBoot you pick it
from the firmware menu, or set a one-shot with `sudo efibootmgr --bootnext <NNNN>`
(the MyBoot entry number) which lasts exactly one boot. Nothing about your
default boot changed.

**Why not `curl | sh` here:** the install script deliberately refuses NixOS,
because an imperative install fights `nixos-rebuild`. The coexistence steps above
sidestep that by living in `\EFI\MyBoot` (a dir nixos-rebuild ignores) plus a
manual NVRAM entry. If you later want MyBoot as the *managed* default, do it
declaratively:

```nix
# configuration.nix — only once MyBoot is proven on your hardware
boot.loader.systemd-boot.enable = false;
boot.loader.grub.enable = false;
boot.loader.external = {
  enable = true;
  installHook = "${myboot-installer}/bin/myboot-install";  # wrap `myboot install`
};
boot.loader.efi.canTouchEfiVariables = true;
```

### 11.5 Rescue plan (have this ready BEFORE §11.4)

- Keep a **NixOS live/installer USB** on hand. If the machine won't boot, boot the
  live USB, `mount /dev/nvme0n1p1 /mnt`, and either delete `/mnt/EFI/MyBoot` or
  just fix BootOrder.
- systemd-boot is still fully installed at `/boot/EFI/systemd` and
  `/boot/EFI/BOOT/BOOTX64.EFI` — MyBoot never removed it. From the firmware boot
  menu you can always pick "Linux Boot Manager" (systemd-boot) directly.
- Remove the MyBoot NVRAM entry any time:
  `sudo efibootmgr` (find the number) → `sudo efibootmgr -b <NNNN> -B`.
- Remove MyBoot's files: `sudo ./host-cli/target/*/release/myboot uninstall --esp /boot`.
- A NixOS `nixos-rebuild switch` will re-assert systemd-boot as the managed
  loader regardless, so you are never more than one rebuild away from a known-good
  boot configuration.

### 11.6 One honest gate before §11.4

Do not install onto the internal disk until §11.1 compiles cleanly and §11.3
(USB boot) has actually booted each of your OSes through MyBoot on this hardware.
The internal-disk step is reversible as described, but "reversible" still means
"you might need the rescue USB" — so earn the confidence on the USB first.

# MyBoot

**An intelligent, transactional UEFI boot manager, written in Rust.**

MyBoot discovers the operating systems on *every* disk at boot time — no config to
generate, ever — presents a menu, and chainloads the one you pick with a
transactional, roll-back-safe lifecycle. Its safety-critical parts are formally
verified (the boot transaction in TLA+, the parsers with Kani).

It boots real Linux distributions today (Ubuntu and Parrot proven end-to-end on
OVMF) and lists any number of OSes across any number of disks.

```
             ┌──────────── UEFI firmware ────────────┐
             │            loads one .efi             │
             └───────────────────┬───────────────────┘
                                 ▼
                              MyBoot
             scan every disk → build Boot Graph → health →
             policy → menu → transactional handoff → your OS
```

---

## Quick install (one line)

**Linux:**

```sh
curl -fsSL https://raw.githubusercontent.com/ctrl-routing-Mosesgarlic/Myboot/main/install.sh | sudo sh
```

**Windows** (PowerShell — prompts for Administrator elevation itself, no `sudo`):

```powershell
irm https://raw.githubusercontent.com/ctrl-routing-Mosesgarlic/Myboot/main/install.ps1 | iex
```

Both downloads the latest released `myboot.efi`, verify its checksum, install it to
`\EFI\MyBoot` on your EFI System Partition (coexisting — it never touches your other
loaders), and register a firmware boot entry. Reboot and pick **MyBoot** from the
firmware menu. Full walkthrough and safety notes: [`INSTALL_GUIDE.md`](INSTALL_GUIDE.md).

> **Not on NixOS.** `nixos-rebuild` reverts imperative bootloaders — use the
> declarative path in [`docs/packaging/NIX.md`](docs/packaging/NIX.md).

---

## Why MyBoot

| | Install | Config to generate | After a kernel/OS change | Discovers other OSes |
|---|---|---|---|---|
| **GRUB** | `grub-install` **+** `grub-mkconfig` | `grub.cfg` | **re-run `grub-mkconfig`** | via `os-prober` |
| **systemd-boot** | `bootctl install` | a hand-written entry per OS | edit entries | no |
| **MyBoot** | one command | **none** | **nothing** | **yes, every boot** |

- **Auto-discovery** — finds Windows, your distros, and NixOS generations across all
  disks by scanning ESPs, vendor directories, BLS entries, and NixOS bootspec.
- **Transactional & never-dead-end** — arms a marker before handoff, confirms or
  rolls back on the next boot; if a chosen OS fails, it falls through to the next
  healthy one instead of stranding you.
- **Multi-disk aware** — volume-scoped entries: the same loader path on two disks
  stays distinct, and MyBoot chainloads the *right* disk's copy via device path.
- **Explainable policy** — deterministic, health-gated selection with a stated
  reason; no ML in the boot path.
- **Three interfaces over one engine** — a graphical menu, a pre-boot shell, and a
  host CLI.
- **Verified core** — TLA+ model of the boot transaction, Kani proofs on the parsers;
  `#![forbid(unsafe_code)]` everywhere except the small firmware/arch adapters.

---

## Build from source

Requires a Rust toolchain with the `x86_64-unknown-uefi` target (a Nix `flake.nix` is
provided; `direnv allow` / `nix develop` gives you everything, including QEMU + OVMF).

```sh
cargo build -p myboot --release --target x86_64-unknown-uefi
file target/x86_64-unknown-uefi/release/myboot.efi   # PE32+ EFI application
```

## Test it (no hardware needed)

The `pyboot` tooling (`./manage.py`) builds, drives QEMU, and asserts on the serial
console:

```sh
./manage.py check                         # host tests + build + smoke test
./manage.py iso-test path/to/distro.iso   # boot a real distro ISO in a QEMU window
./manage.py iso-test a.iso b.iso c.iso    # the n-OS test: many OSes, one menu
```

See [`pyboot/README.md`](pyboot/README.md).

---

## Layout

- `bin/myboot` — the UEFI application (compiles to `BOOTX64.EFI`).
- `crates/*` — the engine, one crate per module (graph, discovery, policy, health,
  transaction, providers, config, storage, ports, security, ui-gfx, ui-shell, arch,
  platform, engine).
- `host-cli` — the host-side management tool for the installed OS.
- `pyboot/` + `manage.py` — build/test/install tooling.
- `packaging/` — AUR `PKGBUILD`, Debian, RPM, Snap, per-store recipes.
- `docs/`, `spec/` — design report, verification (TLA+/Kani), packaging guides.

## Documentation

- [`INSTALL_GUIDE.md`](INSTALL_GUIDE.md) — install on Arch and any UEFI system.
- [`HOW_IT_WORKS.md`](HOW_IT_WORKS.md) — step-by-step, plain-language + technical.
- [`DISTRIBUTION_GUIDE.md`](DISTRIBUTION_GUIDE.md) — roadmap into the distro repos.
- [`UEFI_UPGRADE_NOTES.md`](UEFI_UPGRADE_NOTES.md) — the uefi 0.33 → 0.40 plan.
- `docs/packaging/` — one guide per store (pacman, apt, rpm, snap, nix) + review docs.

---

## Status

Working: builds and runs on real UEFI firmware; discovers, names, and boots real
Linux distros across multiple volumes; transactional handoff and never-dead-end
fallback. Pinned to `uefi` 0.33 (a deliberate, documented choice; see the upgrade
notes). Secure Boot signing and broader installer integration are on the roadmap.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.

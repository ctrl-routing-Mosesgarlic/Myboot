# pyboot — MyBoot build / test / install tooling

This Python package replaces the old shell scripts with one cross-checkable CLI. It
builds MyBoot, drives QEMU, **parses the serial console and asserts pass/fail** (the
pattern QEMU's and U-Boot's own test suites use), and installs to real media with
hard safety guards.

Run everything through the repo-root entry point (inside the dev shell):

```
./manage.py --help
```

## Commands

| Command | What it does | Replaces |
|---|---|---|
| `./manage.py test` | Run the pure-crate host tests (`cargo test` per crate) | `run-tests.sh` |
| `./manage.py build` | Build `myboot.efi` for `x86_64-unknown-uefi` | (part of the others) |
| `./manage.py smoke` | Build → stage a synthetic ESP → boot in QEMU → assert discovery/handoff | `run-qemu.sh` |
| `./manage.py iso-test ISO [ISO ...]` | Open a QEMU **window** to boot one or MORE real distro ISOs — each becomes its own volume; pick any from MyBoot's menu and watch it boot (the n-OS proof) | (new) |
| `./manage.py check` | `test` + `build` + `smoke` — the safe, automated gate | (new) |
| `./manage.py verify` | Kani proofs + TLA+ model check | `verify.sh` |
| `./manage.py install [--esp DIR] [--register]` | Install to this machine's ESP (coexists, never formats) | `install.sh` |
| `./manage.py uninstall [--esp DIR]` | Remove MyBoot from the ESP | `uninstall.sh` |
| `./manage.py usb --device /dev/sdX` | Write to a **removable** USB (guarded, non-destructive) | `make-usb.sh` |

Exit codes: `0` success, `1` a real test/verification failure, `2` an
environment/safety problem, `130` interrupted.

## Design (one responsibility per file)

```
manage.py            repo-root entry point
pyboot/
  cli.py             argparse wiring + dispatch (no logic)
  errors.py          typed exceptions
  console.py         colored status output
  proc.py            subprocess wrapper (no shell strings)
  paths.py           repo/target/artifact/ESP path resolution
  env.py             OVMF / UEFI-shell / host-triple resolution
  events.py          typed model of MyBoot's serial events
  logparse.py        serial text  -> events           (pure)
  assertions.py      events        -> pass/fail checks (pure)
  report.py          checks        -> printed report + exit
  build.py           host tests + UEFI build
  esp.py             stage the synthetic test ESP
  qemu.py            pexpect-driven QEMU run + capture
  smoke.py           compose the smoke test
  iso.py             compose the real-OS ISO test
  install.py         install/uninstall gates + delegate to the Rust host CLI
  usb.py             guarded removable-USB writer
  verify.py          Kani + TLA+ runner
tools/tests/         pytest unit tests for the pure logic (logparse/assertions/cli)
```

The pure logic (`events`, `logparse`, `assertions`) has no I/O and is unit-tested
without QEMU:

```
python -m pytest tools/tests        # or: python tools/tests/test_logparse.py
```

## Safety

- `install` refuses non-UEFI systems and NixOS (declarative), auto-detects the ESP,
  and delegates the actual copy to the tested Rust host CLI so it coexists with
  systemd-boot/GRUB and never formats anything.
- `usb` refuses any device not marked removable (`/sys/block/<dev>/removable`),
  requires an explicit `--device` and a typed confirmation.
- Nothing here ever writes to an internal disk without your explicit action.

## `verify` — optional formal tools

`./manage.py verify` runs Kani (code proofs) and TLC (the TLA+ model check) when
they are installed, and skips (with a clear message) when they are not:

- Kani: `cargo install --locked kani-verifier` (installs `cargo-kani` into
  `~/.cargo/bin`; `verify` looks there automatically).
  - **On NixOS**, Kani's downloaded toolchain is a generic-Linux binary NixOS
    can't run directly. Recommended (free): add `programs.nix-ld.enable = true;`
    to your NixOS config and rebuild, then `cargo kani setup` works and
    `./manage.py verify` runs Kani directly. Or point it at an FHS wrapper you
    have: `MYBOOT_KANI_WRAP='steam-run' ./manage.py verify` (steam-run is unfree,
    so do NOT add it to the flake — it breaks the whole dev shell).
  - On other distros, add `~/.cargo/bin` to PATH and run `cargo kani setup` once.
- TLC: install the TLA+ tools so `tlc` is on PATH. The spec lives at
  `spec/tla/BootTransaction.tla`.

"""Run MyBoot's formal verification: Kani proofs and the TLA+ model check.

ONE job: invoke the verification tools when they are available, and say clearly when
they are not (these are heavy, OPTIONAL tools — the 105 host tests are the real
coverage). Not a stub — it runs the real checks; it degrades gracefully when a tool
is absent.

NixOS note: Kani's `cargo kani setup` downloads a prebuilt generic-Linux Rust
toolchain, which NixOS cannot execute unless a stub dynamic linker is present. The
clean, free fix is `programs.nix-ld` (we auto-detect it via /lib64/ld-linux). A user
may instead point us at an FHS wrapper with the MYBOOT_KANI_WRAP env var.
"""
from __future__ import annotations

import os
from pathlib import Path
from typing import List, Optional

from . import console, proc, paths


def _cargo_bin() -> Path:
    return Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))) / "bin"


def _env_with_cargo_bin() -> dict:
    env = dict(os.environ)
    env["PATH"] = str(_cargo_bin()) + os.pathsep + env.get("PATH", "")
    return env


def _find(tool: str) -> Optional[str]:
    """Locate a tool on PATH or in ~/.cargo/bin."""
    if proc.which(tool):
        return tool
    candidate = _cargo_bin() / tool
    return str(candidate) if candidate.is_file() else None


def _on_nixos() -> bool:
    return Path("/etc/NIXOS").exists() or Path("/run/current-system/nixos-version").is_file()


def _generic_linker_present() -> bool:
    """True if a generic-Linux dynamic loader exists — i.e. nix-ld is active, so
    downloaded toolchains can run directly."""
    return Path("/lib64/ld-linux-x86-64.so.2").exists() or "NIX_LD" in os.environ


def _kani_prefix() -> Optional[List[str]]:
    """Command prefix to run Kani, or None if it cannot run here.

    - ``MYBOOT_KANI_WRAP`` (if set) wins: empty forces plain, otherwise it is an
      FHS wrapper command (e.g. ``steam-run``).
    - Non-NixOS: run plain.
    - NixOS: run plain when a stub linker exists (nix-ld); else use an available
      ``steam-run``; else give up (guidance).
    """
    if _find("cargo-kani") is None:
        return None
    wrap = os.environ.get("MYBOOT_KANI_WRAP")
    if wrap is not None:
        return wrap.split()
    if not _on_nixos():
        return []
    if _generic_linker_present():
        return []
    if proc.which("steam-run"):
        return ["steam-run"]
    return None


def _kani_guidance() -> None:
    if _find("cargo-kani") is None:
        console.warn("Kani not found. Install: `cargo install --locked kani-verifier` "
                     "(puts cargo-kani in ~/.cargo/bin, which `verify` finds automatically).")
        return
    if _on_nixos():
        console.warn("Kani is installed but its downloaded toolchain is a generic-Linux "
                     "binary that NixOS can't run directly. Pick one (Kani is OPTIONAL):")
        console.info("  1) recommended, free: add `programs.nix-ld.enable = true;` to your")
        console.info("     NixOS config and rebuild, then run `cargo kani setup` once.")
        console.info("  2) or point us at an FHS wrapper you have, e.g.:")
        console.info("     MYBOOT_KANI_WRAP='steam-run' ./manage.py verify")
        console.info("  The 105 host tests (`./manage.py test`) are the real coverage.")
    else:
        console.warn("cargo-kani found but it did not run; try `cargo kani setup` "
                     "(downloads its toolchain on first use).")


def _run_kani(root: Path) -> Optional[bool]:
    """Run the Kani proofs. Returns True/False if it ran, or None if skipped."""
    prefix = _kani_prefix()
    if prefix is None:
        _kani_guidance()
        return None
    console.section("Kani — parser proofs (storage)")
    if prefix:
        console.info(f"running Kani via: {' '.join(prefix)}")
    res = proc.run(prefix + ["cargo", "kani", "-p", "storage"],
                   cwd=str(root), env=_env_with_cargo_bin(), check=False)
    if not res.ok and _on_nixos() and not prefix:
        console.warn("Kani failed — if this is a 'cannot execute' error, enable "
                     "programs.nix-ld or use MYBOOT_KANI_WRAP (see `verify` help above).")
    return res.ok


def _run_tlc(root: Path) -> Optional[bool]:
    """Run the TLA+ model check. Returns True/False if it ran, or None if skipped."""
    spec = root / "spec" / "tla" / "BootTransaction.tla"
    cfg = root / "spec" / "tla" / "Bounded.cfg"
    tlc = _find("tlc")
    if tlc is None:
        console.warn("TLC not found; skipping the TLA+ model check "
                     "(install the TLA+ tools so `tlc` is on PATH).")
        return None
    if not (spec.is_file() and cfg.is_file()):
        console.warn(f"TLC is installed but the spec is missing at {spec}; skipping.")
        return None
    console.section("TLC — boot-transaction model check")
    res = proc.run([tlc, str(spec), "-config", str(cfg)], cwd=str(root), check=False)
    return res.ok


def run() -> bool:
    """Run Kani and TLC where available. A missing/unrunnable tool is reported and
    skipped, not counted as a failure."""
    root = paths.repo_root()
    results = [r for r in (_run_kani(root), _run_tlc(root)) if r is not None]

    if not results:
        console.warn("no verification tools available — nothing was checked.")
        return True
    ok = all(results)
    (console.success if ok else console.error)(
        "verification passed" if ok else "verification reported a failure")
    return ok

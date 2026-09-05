"""Resolve the project's important paths.

ONE job: know where things are. Finds the workspace root (the directory whose
``Cargo.toml`` declares ``[workspace]``) by walking up from this file, and derives
the EFI artifact, the ESP staging dir, and the host-CLI path from it. No building,
no I/O beyond reading ``Cargo.toml`` to confirm the root.
"""
from __future__ import annotations

from pathlib import Path
from functools import lru_cache

from .errors import EnvironmentError_

EFI_TARGET = "x86_64-unknown-uefi"
EFI_REL = f"target/{EFI_TARGET}/release/myboot.efi"


@lru_cache(maxsize=1)
def repo_root() -> Path:
    """The workspace root — the ancestor whose Cargo.toml has ``[workspace]``."""
    here = Path(__file__).resolve()
    for parent in [here.parent, *here.parents]:
        cargo = parent / "Cargo.toml"
        if cargo.is_file() and "[workspace]" in cargo.read_text(encoding="utf-8", errors="ignore"):
            return parent
    raise EnvironmentError_(
        "could not locate the MyBoot workspace root (no Cargo.toml with [workspace] "
        "above this file); run pyboot from inside the repository."
    )


def target_dir() -> Path:
    return repo_root() / "target"


def efi_binary() -> Path:
    """The built firmware binary (may not exist yet — build first)."""
    return repo_root() / EFI_REL


def esp_dir() -> Path:
    """Where the synthetic test ESP is staged (git-ignored build area)."""
    return repo_root() / "build" / "esp"


def firmware_dir() -> Path:
    return repo_root() / "firmware"


def host_cli_manifest() -> Path:
    return repo_root() / "host-cli" / "Cargo.toml"

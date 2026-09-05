"""Resolve the runtime environment the harness needs.

ONE job: find the firmware and tools, or fail with a clear, actionable message.
Values come from the dev-shell environment (the flake exports ``OVMF_CODE``,
``OVMF_VARS``, ``MYBOOT_UEFI_SHELL``); nothing is hard-coded to a nix store path.
"""
from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from . import proc
from .errors import EnvironmentError_


@dataclass(frozen=True)
class Env:
    ovmf_code: Path
    ovmf_vars: Path
    host_triple: str
    uefi_shell: Optional[Path]


def _require_env_file(name: str) -> Path:
    val = os.environ.get(name)
    if not val:
        raise EnvironmentError_(
            f"${name} is not set. Enter the dev shell first (`direnv allow` or "
            f"`nix develop`), which exports the OVMF firmware paths."
        )
    p = Path(val)
    if not p.is_file():
        raise EnvironmentError_(f"${name} points at a missing file: {p}")
    return p


def host_triple() -> str:
    """The host target triple, from ``rustc -vV`` (e.g. x86_64-unknown-linux-gnu)."""
    res = proc.run(["rustc", "-vV"], capture=True, echo=False)
    for line in res.stdout.splitlines():
        if line.startswith("host: "):
            return line[len("host: "):].strip()
    raise EnvironmentError_("could not determine the host triple from `rustc -vV`.")


def _optional_shell() -> Optional[Path]:
    val = os.environ.get("MYBOOT_UEFI_SHELL")
    if val and Path(val).is_file():
        return Path(val)
    return None


def resolve() -> Env:
    """Resolve the full environment, checking required tools are present."""
    for tool in ("cargo", "rustc", "qemu-system-x86_64"):
        if not proc.which(tool):
            raise EnvironmentError_(
                f"required tool '{tool}' not found on PATH; enter the dev shell first."
            )
    return Env(
        ovmf_code=_require_env_file("OVMF_CODE"),
        ovmf_vars=_require_env_file("OVMF_VARS"),
        host_triple=host_triple(),
        uefi_shell=_optional_shell(),
    )


def real_loader() -> Optional[Path]:
    """A real, different EFI app to chainload in tests, if one is available.

    Order: explicit override, the UEFI shell from the flake, the host's systemd-boot.
    """
    override = os.environ.get("MYBOOT_REAL_LOADER")
    if override and Path(override).is_file():
        return Path(override)
    shell = _optional_shell()
    if shell is not None:
        return shell
    sd = Path("/boot/EFI/systemd/systemd-bootx64.efi")
    if sd.is_file():
        return sd
    return None

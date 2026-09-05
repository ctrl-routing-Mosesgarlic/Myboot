"""Stage the synthetic EFI System Partition used by the QEMU tests.

ONE job: lay out a directory that QEMU serves as a FAT volume, containing MyBoot
(as the fallback loader and under ``\\EFI\\MyBoot``) plus a few discoverable "OS"
loaders. One entry uses a real, different EFI application when available, so a
handoff can be exercised end to end.
"""
from __future__ import annotations

import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

from . import console, paths

_CONFIG = """\
default = "windows:default"
timeout_secs = 8
policy = "last-good-then-default"
confirm = true
max_tries = 3
rollback = "last-good"
"""

# ISO test: a dynamic default that matches no decoy (there are none), so policy
# picks the healthiest real entry; longer timeout so you can read the menu and
# pick the ISO.
_CONFIG_MINIMAL = """\
default = "auto"
timeout_secs = 20
policy = "last-good-then-default"
confirm = true
max_tries = 3
rollback = "last-good"
"""


@dataclass(frozen=True)
class StagedEsp:
    root: Path
    real_loader_used: Optional[Path]


def stage(efi_binary: Path, real_loader: Optional[Path], *, minimal: bool = False) -> StagedEsp:
    """Create the synthetic ESP under ``build/esp`` and return where it is.

    Default: stage a few discoverable "OS" decoys (backed by a real EFI app) for the
    self-contained SMOKE test. ``minimal=True``: stage ONLY MyBoot (fallback + its
    own dir) — used by the ISO test so the real OS on the attached ISO is the only
    real target and the menu isn't polluted with decoys.
    """
    esp = paths.esp_dir()
    if esp.exists():
        shutil.rmtree(esp)

    if minimal:
        (esp / "EFI/BOOT").mkdir(parents=True, exist_ok=True)
        (esp / "EFI/MyBoot").mkdir(parents=True, exist_ok=True)
        # MyBoot at the removable fallback path so firmware boots it, plus its own
        # dir. No decoys: the only real OS is whatever is on the attached media.
        shutil.copy2(efi_binary, esp / "EFI/BOOT/BOOTX64.EFI")
        shutil.copy2(efi_binary, esp / "EFI/MyBoot/BOOTX64.EFI")
        (esp / "EFI/MyBoot/config.toml").write_text(_CONFIG_MINIMAL, encoding="utf-8")
        console.success(f"staged minimal ESP (MyBoot only) at {esp.relative_to(paths.repo_root())}")
        return StagedEsp(root=esp, real_loader_used=None)

    for sub in ("EFI/BOOT", "EFI/MyBoot", "EFI/Microsoft/Boot", "EFI/ubuntu"):
        (esp / sub).mkdir(parents=True, exist_ok=True)

    # The default-picked entry (Windows, healthiest-first) gets the REAL loader so
    # the first handoff launches a genuine, different EFI app (the shell) and stops
    # — rather than a myboot.efi self-copy, which would chainload MyBoot into itself
    # forever and never reach a real target. Ubuntu also gets it, so the test is
    # robust to graph-order changes; the \EFI\BOOT fallback stays MyBoot (it is the
    # boot medium).
    shutil.copy2(efi_binary, esp / "EFI/BOOT/BOOTX64.EFI")
    shutil.copy2(efi_binary, esp / "EFI/MyBoot/BOOTX64.EFI")
    shutil.copy2(efi_binary, esp / "EFI/ubuntu/grubx64.efi")

    if real_loader is not None:
        shutil.copy2(real_loader, esp / "EFI/Microsoft/Boot/bootmgfw.efi")
        shutil.copy2(real_loader, esp / "EFI/ubuntu/shimx64.efi")
        console.info(f"real chainload target for the default entry: {real_loader}")
    else:
        shutil.copy2(efi_binary, esp / "EFI/Microsoft/Boot/bootmgfw.efi")
        shutil.copy2(efi_binary, esp / "EFI/ubuntu/shimx64.efi")
        console.info("no real loader found; entries are myboot.efi self-copies")

    (esp / "EFI/ubuntu/os-release").write_text(
        'NAME=Ubuntu\nPRETTY_NAME="Ubuntu 24.04 LTS"\n', encoding="utf-8"
    )
    (esp / "EFI/MyBoot/config.toml").write_text(_CONFIG, encoding="utf-8")

    console.success(f"staged ESP at {esp.relative_to(paths.repo_root())}")
    return StagedEsp(root=esp, real_loader_used=real_loader)

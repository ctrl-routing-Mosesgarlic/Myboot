"""Install and uninstall MyBoot on this machine's ESP.

ONE job: the environment gates and orchestration around installation. The default
source is a GitHub release (how end users install); `--from build` builds locally.
The actual on-disk layout is deliberately minimal and coexisting — MyBoot only ever
writes `<ESP>/EFI/MyBoot/` and, with `--register`, adds ONE firmware boot entry. It
NEVER formats a disk or removes another OS's loader.
"""
from __future__ import annotations

import os
import re
import shutil
import tempfile
from pathlib import Path
from typing import Optional, Tuple

from . import build, console, proc, github
from .env import Env
from .errors import EnvironmentError_, SafetyError

_INSTALL_SUBDIR = "EFI/MyBoot"
_DEFAULT_CONFIG = """\
# MyBoot configuration (created by `manage.py install`; safe to edit).
default = "auto"
timeout_secs = 5
policy = "last-good-then-default"
confirm = true
max_tries = 3
rollback = "last-good"
"""


def _gate_root() -> None:
    if hasattr(os, "geteuid") and os.geteuid() != 0:
        raise SafetyError("run as root (use sudo); installation writes to the ESP.")


def _gate_uefi() -> None:
    if not Path("/sys/firmware/efi").is_dir():
        raise SafetyError("this is not a UEFI system; MyBoot is UEFI-only.")


def _gate_not_nixos() -> None:
    if Path("/etc/NIXOS").exists() or Path("/run/current-system/nixos-version").is_file():
        raise SafetyError(
            "NixOS detected. Do NOT install a bootloader imperatively — nixos-rebuild "
            "will revert it. Use the declarative boot.loader.external approach "
            "(see INSTALL_GUIDE.md)."
        )


def _detect_esp(explicit: Optional[str]) -> Path:
    if explicit:
        esp = Path(explicit)
        if not (esp / "EFI").is_dir():
            raise EnvironmentError_(f"{esp} has no EFI/ directory — is it the ESP?")
        return esp
    for cand in ("/boot/efi", "/boot", "/efi"):
        if (Path(cand) / "EFI").is_dir():
            return Path(cand)
    raise EnvironmentError_("could not find the ESP; pass --esp <mountpoint> (e.g. /boot).")


def esp_disk_and_part(esp: Path) -> Tuple[str, str]:
    """Resolve the ESP's block device into (disk, partition-number) for efibootmgr,
    e.g. /dev/nvme0n1p1 -> ('/dev/nvme0n1', '1'); /dev/sda1 -> ('/dev/sda', '1')."""
    res = proc.run(["findmnt", "-no", "SOURCE", str(esp)], capture=True, check=False, echo=False)
    src = res.stdout.strip()
    if not src.startswith("/dev/"):
        raise EnvironmentError_(f"could not determine the ESP device for {esp} (got '{src}').")
    return split_disk_partition(src)


def split_disk_partition(dev: str) -> Tuple[str, str]:
    """Split a partition device into (disk, partition-number). Handles the p-suffix
    naming of nvme/mmcblk/loop as well as plain sdXN."""
    base = dev
    m = re.search(r"(p)?(\d+)$", base)
    if not m:
        raise EnvironmentError_(f"{dev} does not look like a partition device.")
    part = m.group(2)
    if m.group(1) == "p" or re.search(r"(nvme\d+n\d+|mmcblk\d+|loop\d+)p\d+$", base):
        disk = base[: base.rfind("p")]
    else:
        disk = base[: -len(part)]
    return disk, part


def _place_on_esp(esp: Path, efi: Path) -> None:
    install_dir = esp / _INSTALL_SUBDIR
    install_dir.mkdir(parents=True, exist_ok=True)
    shutil.copy2(efi, install_dir / "BOOTX64.EFI")
    cfg = install_dir / "config.toml"
    if cfg.exists():
        console.info(f"config exists, left unchanged: {cfg}")
    else:
        cfg.write_text(_DEFAULT_CONFIG, encoding="utf-8")
        console.info(f"created config: {cfg}")


def _register(esp: Path) -> None:
    disk, part = esp_disk_and_part(esp)
    proc.run([
        "efibootmgr", "--create", "--disk", disk, "--part", part,
        "--loader", r"\EFI\MyBoot\BOOTX64.EFI", "--label", "MyBoot", "--unicode",
    ])
    console.success("registered a 'MyBoot' firmware boot entry")


def install(env: Env, *, esp: Optional[str], register: bool,
            source: str = "github", repo: Optional[str] = None,
            tag: Optional[str] = None) -> None:
    """Install MyBoot onto the ESP, coexisting with existing loaders."""
    _gate_root()
    _gate_uefi()
    _gate_not_nixos()

    esp_path = _detect_esp(esp)
    console.step(f"using ESP: {esp_path}")

    if source == "build":
        efi = build.build_efi()
        _place_on_esp(esp_path, efi)
    else:  # github (default)
        repo_slug = github.resolve_repo(repo)
        tmp = Path(tempfile.mkdtemp(prefix="myboot-dl-"))
        try:
            efi = github.fetch_efi(repo_slug, tmp, tag=tag)
            _place_on_esp(esp_path, efi)
        finally:
            shutil.rmtree(tmp, ignore_errors=True)

    if register:
        _register(esp_path)
    else:
        disk, part = esp_disk_and_part(esp_path)
        console.info("to make MyBoot selectable in the firmware, register it:")
        console.info(f"  sudo efibootmgr --create --disk {disk} --part {part} "
                     r"--loader '\EFI\MyBoot\BOOTX64.EFI' --label 'MyBoot' --unicode")
    console.success(f"MyBoot installed to {esp_path}/EFI/MyBoot (other loaders untouched)")


def uninstall(env: Env, *, esp: Optional[str]) -> None:
    """Remove MyBoot from the ESP (its files and, if present, its boot entry)."""
    _gate_root()
    _gate_uefi()
    esp_path = _detect_esp(esp)
    target = esp_path / _INSTALL_SUBDIR
    if target.exists():
        shutil.rmtree(target)
        console.success(f"removed {target}")
    else:
        console.info(f"nothing to remove at {target}")
    console.info("if you registered a firmware entry, delete it with: "
                 "sudo efibootmgr -b <NNNN> -B  (find 'MyBoot' in `efibootmgr`)")

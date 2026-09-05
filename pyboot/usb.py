"""Write MyBoot to a removable USB stick for real-hardware testing.

ONE job: put ``myboot.efi`` on a FAT USB as the removable-media fallback loader,
with hard safety guards so a system disk can never be targeted. It refuses any
device that is not marked removable, requires an explicit device and a typed
confirmation, and uses partition tools (never a raw image ``dd``).
"""
from __future__ import annotations

import os
from pathlib import Path

from . import build, console, proc
from .errors import SafetyError


def _is_block_device(dev: str) -> bool:
    try:
        import stat
        return stat.S_ISBLK(os.stat(dev).st_mode)
    except OSError:
        return False


def _is_removable(dev: str) -> bool:
    base = Path(dev).name
    flag = Path(f"/sys/block/{base}/removable")
    try:
        return flag.read_text().strip() == "1"
    except OSError:
        return False


def make_usb(device: str, *, assume_yes: bool = False) -> None:
    """Format ``device`` FAT32 and install MyBoot's removable-media loader.

    Guards, in order: root, block device, removable flag, typed confirmation.
    """
    if hasattr(os, "geteuid") and os.geteuid() != 0:
        raise SafetyError("run as root (sudo); writing a USB needs privileges.")
    if not _is_block_device(device):
        raise SafetyError(f"{device} is not a block device.")
    if not _is_removable(device):
        raise SafetyError(
            f"REFUSING: {device} is not marked removable "
            f"(/sys/block/{Path(device).name}/removable != 1). This protects your "
            f"system disk. Only removable USB media is allowed."
        )

    console.warn(f"About to ERASE and reformat {device}. All data on it will be lost.")
    if not assume_yes:
        confirm = input(f"Type the device path exactly to proceed [{device}]: ").strip()
        if confirm != device:
            raise SafetyError("confirmation did not match; aborting.")

    efi = build.build_efi()

    part = f"{device}1"
    console.step(f"partitioning {device} (GPT, one ESP partition)")
    proc.run(["parted", "-s", device, "mklabel", "gpt"])
    proc.run(["parted", "-s", device, "mkpart", "ESP", "fat32", "1MiB", "100%"])
    proc.run(["parted", "-s", device, "set", "1", "esp", "on"])
    console.step(f"formatting {part} as FAT32")
    proc.run(["mkfs.vfat", "-F32", "-n", "MYBOOT", part])

    mnt = Path("/tmp/myboot-usb-mnt")
    mnt.mkdir(parents=True, exist_ok=True)
    try:
        proc.run(["mount", part, str(mnt)])
        (mnt / "EFI/BOOT").mkdir(parents=True, exist_ok=True)
        (mnt / "EFI/MyBoot").mkdir(parents=True, exist_ok=True)
        import shutil
        shutil.copy2(efi, mnt / "EFI/BOOT/BOOTX64.EFI")
        shutil.copy2(efi, mnt / "EFI/MyBoot/BOOTX64.EFI")
        proc.run(["sync"])
    finally:
        proc.run(["umount", str(mnt)], check=False)

    console.success(
        f"MyBoot written to {device}. Boot it from your firmware's boot menu "
        f"(F12/F9/Esc). It is non-destructive: remove the USB to revert."
    )

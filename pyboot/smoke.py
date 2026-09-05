"""The QEMU smoke test: does MyBoot discover, present, and hand off?

ONE job: compose build → stage → run → parse → assert into the self-contained
smoke test (no real OS needed). Proves the machinery; it does not prove a real OS
boots (that is ``iso``).
"""
from __future__ import annotations

from . import build, esp, qemu, logparse, assertions as A, report, console
from .env import Env, real_loader
from .errors import AssertionFailure


def run(env: Env, *, timeout: int = 40) -> None:
    """Build, stage a synthetic ESP, boot it in QEMU, and assert the pipeline ran."""
    efi = build.build_efi()
    staged = esp.stage(efi, real_loader())

    result = qemu.run(
        env, staged.root, timeout=timeout,
        stop_on=[r"UEFI Interactive Shell", r"no candidate could be booted"],
    )
    events = logparse.parse(result.output)

    checks = [
        A.scanned_at_least(events, 1),
        A.discovered_bootable(events, 2),  # Windows + Ubuntu decoys (MyBoot self-entry now excluded)
        A.has_entry(events, "Windows"),
        A.has_entry(events, "Ubuntu"),
        A.any_handoff(events),
        # The real proof, and console-independent: MyBoot handed off and control
        # never returned (a successful StartImage never returns). Works headless.
        A.handoff_took_over(events, result.output),
    ]

    ok = report.render("smoke test", checks)
    console.info("note: the smoke test proves discovery + menu + handoff, NOT that a "
                 "real OS boots. Use `iso-test` with a real ISO for that.")
    if not ok:
        raise AssertionFailure("smoke test failed")

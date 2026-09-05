"""Assertions over parsed events: the pass/fail logic of the test harness.

ONE job: given the events (and, where unavoidable, the raw capture), decide whether
an expectation held, and return a structured result. Pure — no QEMU, no I/O — so it
is unit-tested directly. ``smoke`` and ``iso`` compose these into a test.
"""
from __future__ import annotations

from dataclasses import dataclass
from typing import List, Optional

from . import events as ev


@dataclass(frozen=True)
class Check:
    """The outcome of one assertion."""
    name: str
    passed: bool
    detail: str


def _first(events: List[object], kind) -> Optional[object]:
    for e in events:
        if isinstance(e, kind):
            return e
    return None


def _all(events: List[object], kind) -> List[object]:
    return [e for e in events if isinstance(e, kind)]


def scanned_at_least(events: List[object], minimum: int) -> Check:
    e = _first(events, ev.VolumeScan)
    if e is None:
        return Check("volume scan", False, "no 'scanning N volume(s)' line seen")
    ok = e.count >= minimum
    return Check("volume scan", ok, f"scanned {e.count} volume(s) (need >= {minimum})")


def discovered_bootable(events: List[object], minimum: int) -> Check:
    e = _first(events, ev.Discovered)
    if e is None:
        return Check("discovery", False, "no 'discovered N entries' line seen")
    ok = e.bootable >= minimum
    return Check(
        "discovery", ok,
        f"discovered {e.total} entr(y/ies), {e.bootable} bootable (need >= {minimum})",
    )


def has_entry(events: List[object], title_substr: str) -> Check:
    hits = [e for e in _all(events, ev.Entry) if title_substr.lower() in e.title.lower()]
    ok = len(hits) > 0
    detail = f"found entry matching '{title_substr}'" if ok else \
        f"no discovered entry matched '{title_substr}'"
    return Check(f"entry '{title_substr}'", ok, detail)


def handed_off_to(events: List[object], id_substr: str) -> Check:
    hits = [e for e in _all(events, ev.HandOff) if id_substr.lower() in e.entry_id.lower()]
    ok = len(hits) > 0
    detail = f"handed off to an id matching '{id_substr}'" if ok else \
        f"never handed off to an id matching '{id_substr}'"
    return Check(f"handoff '{id_substr}'", ok, detail)


def any_handoff(events: List[object]) -> Check:
    hits = _all(events, ev.HandOff)
    ok = len(hits) > 0
    return Check("any handoff", ok, f"{len(hits)} handoff attempt(s) made")


def reached_text(capture: str, needle: str, name: str) -> Check:
    ok = needle.lower() in capture.lower()
    detail = f"saw '{needle}' in output" if ok else f"never saw '{needle}' in output"
    return Check(name, ok, detail)


def no_dead_end(events: List[object]) -> Check:
    """The engine must not report a hard dead-end (`no candidate could be booted`).

    A per-attempt HandoffFailed is fine (that IS the fallback working); a terminal
    NoCandidate means every option was exhausted.
    """
    nc = _first(events, ev.NoCandidate)
    ok = nc is None
    detail = "no terminal dead-end" if ok else f"dead-ended after {nc.tried} tries"
    return Check("never dead-end", ok, detail)


def handoff_stuck(events: List[object], id_substr: str) -> Check:
    """For the ISO test: the chosen loader must NOT immediately bounce back.

    If a handoff to the target is followed by a HandoffFailed for that same target,
    the loader did not take over (e.g. a self-contained app that just exited, or a
    load error). A real OS handoff never returns, so we assert the target was handed
    off AND did not report a failure for it.
    """
    handed = [e for e in _all(events, ev.HandOff) if id_substr.lower() in e.entry_id.lower()]
    failed = [e for e in _all(events, ev.HandoffFailed) if id_substr.lower() in e.entry_id.lower()]
    if not handed:
        return Check(f"boot '{id_substr}'", False, f"never handed off to '{id_substr}'")
    ok = len(failed) == 0
    detail = f"handed off to '{id_substr}' and it did not return" if ok else \
        f"'{id_substr}' was handed off but bounced back ({failed[0].error})"
    return Check(f"boot '{id_substr}'", ok, detail)


def handoff_took_over(events: List[object], capture: str) -> Check:
    """Prove a handoff SUCCEEDED without depending on the child's console output.

    A successful UEFI ``StartImage`` never returns, so MyBoot logs nothing after it.
    Therefore a real handoff shows up as: at least one HandOff, no terminal
    NoCandidate, and the *last* MyBoot event being a HandOff (control was
    transferred and never came back). If the child's banner did reach the serial
    capture, that's even stronger proof and also passes. This is robust to headless
    mode, where a chainloaded app's output may not reach the captured serial.
    """
    if "uefi interactive shell" in capture.lower():
        return Check("handoff took over", True, "chainloaded app launched (shell banner seen)")
    handoffs = _all(events, ev.HandOff)
    if not handoffs:
        return Check("handoff took over", False, "no handoff was attempted")
    if _first(events, ev.NoCandidate) is not None:
        return Check("handoff took over", False, "every candidate failed (dead-ended)")
    last = events[-1]
    if isinstance(last, ev.HandOff):
        return Check("handoff took over", True,
                     f"handed off to '{last.entry_id}' and never returned (child took over)")
    if isinstance(last, ev.HandoffFailed):
        return Check("handoff took over", False, f"last handoff failed ({last.error})")
    return Check("handoff took over", False,
                 f"handed off but continued logging (last event: {type(last).__name__})")


def all_passed(checks: List[Check]) -> bool:
    return all(c.passed for c in checks)

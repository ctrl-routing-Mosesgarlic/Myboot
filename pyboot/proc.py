"""A thin, safe wrapper around subprocess.

ONE job: run an external command, optionally capture it, and raise a typed error on
failure. Every shell-out in pyboot goes through here, so logging and error handling
are uniform and there is no ``shell=True`` string interpolation anywhere.
"""
from __future__ import annotations

import subprocess
from dataclasses import dataclass
from typing import Mapping, Optional, Sequence

from . import console
from .errors import BuildError


@dataclass(frozen=True)
class Result:
    code: int
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        return self.code == 0


def run(
    cmd: Sequence[str],
    *,
    cwd: Optional[str] = None,
    env: Optional[Mapping[str, str]] = None,
    capture: bool = False,
    check: bool = True,
    echo: bool = True,
) -> Result:
    """Run ``cmd`` (a list — never a shell string).

    ``capture=True`` returns stdout/stderr as text; otherwise output streams to the
    terminal. ``check=True`` raises :class:`BuildError` on a non-zero exit.
    """
    if echo:
        console.info("$ " + " ".join(cmd))
    proc = subprocess.run(
        list(cmd),
        cwd=cwd,
        env=dict(env) if env is not None else None,
        text=True,
        capture_output=capture,
    )
    result = Result(
        code=proc.returncode,
        stdout=proc.stdout or "" if capture else "",
        stderr=proc.stderr or "" if capture else "",
    )
    if check and not result.ok:
        detail = result.stderr.strip() or result.stdout.strip() or f"exit {result.code}"
        raise BuildError(f"command failed ({' '.join(cmd)}): {detail}")
    return result


def which(tool: str) -> bool:
    """True if ``tool`` is on PATH."""
    from shutil import which as _which
    return _which(tool) is not None

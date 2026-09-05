"""Console output helpers: colored, structured status lines for the CLI.

ONE job: present messages to the user consistently. Honors ``NO_COLOR`` and a
non-tty stdout (colors are disabled automatically), so output is clean in CI logs.
"""
from __future__ import annotations

import os
import sys

_ENABLED = sys.stdout.isatty() and os.environ.get("NO_COLOR") is None

_BLUE = "\033[1;34m"
_GREEN = "\033[1;32m"
_YELLOW = "\033[1;33m"
_RED = "\033[1;31m"
_DIM = "\033[2m"
_RESET = "\033[0m"


def _c(color: str, text: str) -> str:
    return f"{color}{text}{_RESET}" if _ENABLED else text


def step(msg: str) -> None:
    print(_c(_BLUE, f"[myboot] {msg}"))


def info(msg: str) -> None:
    print(_c(_DIM, f"         {msg}"))


def success(msg: str) -> None:
    print(_c(_GREEN, f"[ ok ]  {msg}"))


def warn(msg: str) -> None:
    print(_c(_YELLOW, f"[warn]  {msg}"))


def error(msg: str) -> None:
    print(_c(_RED, f"[fail]  {msg}"), file=sys.stderr)


def section(title: str) -> None:
    line = "─" * max(4, 60 - len(title))
    print(_c(_BLUE, f"── {title} {line}"))

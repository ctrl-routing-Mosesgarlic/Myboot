"""Exception types for the pyboot tooling.

One place for every error the CLI can raise, so callers can distinguish a
user/environment problem (exit 2) from a genuine test failure (exit 1).
"""
from __future__ import annotations


class PybootError(Exception):
    """Base class for all pyboot errors."""


class EnvironmentError_(PybootError):
    """A required tool, file, or environment variable is missing or wrong.

    Named with a trailing underscore to avoid shadowing the builtin
    ``EnvironmentError``.
    """


class BuildError(PybootError):
    """A cargo build or host-test invocation failed."""


class QemuError(PybootError):
    """QEMU could not be launched or died unexpectedly."""


class SafetyError(PybootError):
    """A requested action was refused because it is unsafe.

    Raised, for example, when a USB target is not a removable device, so the
    tooling never risks writing to a system disk.
    """


class AssertionFailure(PybootError):
    """A test assertion did not hold. Represents a real product failure."""

"""Fetch a released MyBoot binary from GitHub — the way end users will install it.

ONE job: given a repo (``owner/name``) and optionally a tag, find the ``myboot.efi``
release asset and download it (verifying its SHA-256 if a checksum asset is
published). Uses only the standard library, so no extra dependency. The public repo
isn't published yet, so the repo must be supplied via ``--repo`` or ``MYBOOT_REPO``
until it is.
"""
from __future__ import annotations

import hashlib
import json
import os
import shutil
import urllib.request
from pathlib import Path

from . import console
from .errors import EnvironmentError_

# Placeholder until the public repo exists; overridden by --repo / MYBOOT_REPO.
DEFAULT_REPO = "ctrl-routing-Mosesgarlic/Myboot"
ASSET = "myboot.efi"
_UA = {"User-Agent": "pyboot-installer"}


def resolve_repo(explicit: str | None) -> str:
    return explicit or os.environ.get("MYBOOT_REPO") or DEFAULT_REPO


def _api(url: str) -> dict:
    req = urllib.request.Request(
        url, headers={**_UA, "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(req, timeout=30) as r:  # noqa: S310 (trusted host)
        return json.load(r)


def _asset_url(repo: str, name: str, tag: str | None) -> str | None:
    base = f"https://api.github.com/repos/{repo}/releases"
    data = _api(f"{base}/tags/{tag}" if tag else f"{base}/latest")
    for a in data.get("assets", []):
        if a.get("name") == name:
            return a.get("browser_download_url")
    return None


def _download(url: str, dest: Path) -> None:
    req = urllib.request.Request(url, headers=_UA)
    with urllib.request.urlopen(req, timeout=120) as r, open(dest, "wb") as f:  # noqa: S310
        shutil.copyfileobj(r, f)


def _sha256(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def fetch_efi(repo: str, dest_dir: Path, *, tag: str | None = None) -> Path:
    """Download ``myboot.efi`` from the given release into ``dest_dir`` and return
    its path, verifying the checksum if one is published."""
    url = _asset_url(repo, ASSET, tag)
    if url is None:
        raise EnvironmentError_(
            f"release '{tag or 'latest'}' of {repo} has no '{ASSET}' asset.")
    dest = dest_dir / ASSET
    console.step(f"downloading {ASSET} from {repo} ({tag or 'latest'})")
    _download(url, dest)

    sha_url = _asset_url(repo, ASSET + ".sha256", tag)
    if sha_url:
        sha_file = dest_dir / (ASSET + ".sha256")
        _download(sha_url, sha_file)
        expected = sha_file.read_text().split()[0].strip().lower()
        actual = _sha256(dest)
        if actual != expected:
            raise EnvironmentError_(
                f"checksum mismatch for {ASSET}: expected {expected}, got {actual}.")
        console.success("checksum verified")
    else:
        console.warn("no .sha256 checksum asset published; skipping verification")
    return dest

#!/bin/sh
# MyBoot one-line installer.
#
#   curl -fsSL https://raw.githubusercontent.com/ctrl-routing-Mosesgarlic/Myboot/main/install.sh | sudo sh
#
# Downloads the latest released myboot.efi from GitHub, verifies its checksum,
# installs it to <ESP>/EFI/MyBoot/BOOTX64.EFI, writes a default config (only if none
# exists), and registers a firmware boot entry. It NEVER formats a disk or removes
# another OS's loader — it only writes \EFI\MyBoot and adds one boot entry.
#
# Pure POSIX sh + curl (or wget) + efibootmgr. No Python, no repo checkout needed.
# This is the standard model (rustup, nix): a small shell bootstrap for a binary.
#
# Options via environment:
#   MYBOOT_REPO=owner/name   override the source repo (default below)
#   MYBOOT_TAG=v0.1.0        install a specific release (default: latest)
#   MYBOOT_ESP=/boot         override ESP auto-detection
#   MYBOOT_NO_REGISTER=1     skip the efibootmgr entry (just place the files)
#   MYBOOT_MAKE_DEFAULT=1    put MyBoot first in BootOrder (default: leave it last)
set -eu

REPO="${MYBOOT_REPO:-ctrl-routing-Mosesgarlic/Myboot}"
TAG="${MYBOOT_TAG:-latest}"
LABEL="MyBoot"
SUBDIR="EFI/MyBoot"

say()  { printf '\033[1;34m[myboot]\033[0m %s\n' "$*"; }
ok()   { printf '\033[1;32m[ ok ]\033[0m  %s\n' "$*"; }
warn() { printf '\033[1;33m[warn]\033[0m  %s\n' "$*"; }
die()  { printf '\033[1;31m[fail]\033[0m  %s\n' "$*" >&2; exit 1; }

# --- preflight ---------------------------------------------------------------
[ "$(id -u)" -eq 0 ] || die "run as root (pipe into 'sudo sh')."
[ -d /sys/firmware/efi ] || die "not a UEFI system; MyBoot is UEFI-only."
if [ -e /etc/NIXOS ] || [ -f /run/current-system/nixos-version ]; then
  die "NixOS detected — do not install imperatively (nixos-rebuild reverts it). Use the declarative boot.loader.external approach; see docs/packaging/NIX.md."
fi

if command -v curl >/dev/null 2>&1; then DL="curl -fsSL"; DLO="curl -fsSL -o";
elif command -v wget >/dev/null 2>&1; then DL="wget -qO-";  DLO="wget -qO";
else die "need curl or wget."; fi
command -v efibootmgr >/dev/null 2>&1 || warn "efibootmgr not found — will place files but cannot register a boot entry (install it, e.g. 'pacman -S efibootmgr')."

# --- locate the ESP ----------------------------------------------------------
ESP="${MYBOOT_ESP:-}"
if [ -z "$ESP" ]; then
  for c in /boot/efi /boot /efi; do
    if [ -d "$c/EFI" ]; then ESP="$c"; break; fi
  done
fi
[ -n "$ESP" ] && [ -d "$ESP/EFI" ] || die "could not find the EFI System Partition; set MYBOOT_ESP=/your/esp."
say "using ESP: $ESP"

# --- resolve the release asset URL ------------------------------------------
API="https://api.github.com/repos/$REPO/releases"
if [ "$TAG" = "latest" ]; then META_URL="$API/latest"; else META_URL="$API/tags/$TAG"; fi
say "resolving myboot.efi from $REPO ($TAG)"
META="$($DL "$META_URL")" || die "could not reach GitHub API for $REPO."
# extract browser_download_url for myboot.efi and its .sha256 (no jq dependency)
EFI_URL="$(printf '%s' "$META" | grep -o '"browser_download_url": *"[^"]*myboot\.efi"' | head -n1 | sed 's/.*"\(https[^"]*\)"/\1/')"
SHA_URL="$(printf '%s' "$META" | grep -o '"browser_download_url": *"[^"]*myboot\.efi\.sha256"' | head -n1 | sed 's/.*"\(https[^"]*\)"/\1/')"
[ -n "$EFI_URL" ] || die "release '$TAG' has no myboot.efi asset. Has a release been published with build artifacts?"

# --- download + verify -------------------------------------------------------
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
say "downloading myboot.efi"
$DLO "$TMP/myboot.efi" "$EFI_URL" || die "download failed."
if [ -n "$SHA_URL" ] && command -v sha256sum >/dev/null 2>&1; then
  $DLO "$TMP/myboot.efi.sha256" "$SHA_URL" || die "checksum download failed."
  EXPECT="$(awk '{print $1}' "$TMP/myboot.efi.sha256")"
  ACTUAL="$(sha256sum "$TMP/myboot.efi" | awk '{print $1}')"
  [ "$EXPECT" = "$ACTUAL" ] || die "checksum mismatch (expected $EXPECT, got $ACTUAL)."
  ok "checksum verified"
else
  warn "no checksum available; skipping verification."
fi

# --- place on the ESP (coexisting) ------------------------------------------
DEST="$ESP/$SUBDIR"
mkdir -p "$DEST"
cp "$TMP/myboot.efi" "$DEST/BOOTX64.EFI"
if [ ! -e "$DEST/config.toml" ]; then
  cat > "$DEST/config.toml" <<'CFG'
# MyBoot configuration (created by install.sh; safe to edit).
default = "auto"
timeout_secs = 5
policy = "last-good-then-default"
confirm = true
max_tries = 3
rollback = "last-good"
CFG
  say "created $DEST/config.toml"
else
  say "config exists, left unchanged"
fi
ok "MyBoot placed at $DEST (other loaders untouched)"

# --- register the firmware entry --------------------------------------------
register() {
  if efibootmgr | grep -q "^Boot[0-9A-F]\{4\}.* $LABEL\$"; then
    ok "a '$LABEL' firmware boot entry already exists; leaving it (updating the file is enough)"
  else
    SRC="$(findmnt -no SOURCE "$ESP" 2>/dev/null || true)"
    case "$SRC" in /dev/*) : ;; *) warn "could not determine ESP device; register manually (see below)."; return 1;; esac
    case "$SRC" in
      *[0-9]) ;; *) warn "'$SRC' has no partition number; register manually."; return 1;;
    esac
    # split disk + partition number (handles nvme/mmcblk pN and sdXN)
    case "$SRC" in
      *nvme*p[0-9]*|*mmcblk*p[0-9]*|*loop*p[0-9]*)
        PART="${SRC##*p}"; DISK="${SRC%p*}";;
      *) PART="$(printf '%s' "$SRC" | sed 's/.*[^0-9]//')"; DISK="$(printf '%s' "$SRC" | sed 's/[0-9]*$//')";;
    esac
    efibootmgr --create --disk "$DISK" --part "$PART" \
      --loader '\EFI\MyBoot\BOOTX64.EFI' --label "$LABEL" --unicode >/dev/null
    ok "registered a '$LABEL' firmware boot entry (disk=$DISK part=$PART)"
  fi
  if [ "${MYBOOT_MAKE_DEFAULT:-0}" = "1" ]; then
    NUM="$(efibootmgr | sed -n "s/^Boot\([0-9A-F]\{4\}\).* $LABEL\$/\1/p" | head -n1)"
    REST="$(efibootmgr | sed -n 's/^BootOrder: //p' | tr ',' '\n' | grep -v "^$NUM$" | paste -sd, -)"
    [ -n "$NUM" ] && efibootmgr --bootorder "${NUM}${REST:+,$REST}" >/dev/null && ok "MyBoot set first in BootOrder"
  fi
}

if [ "${MYBOOT_NO_REGISTER:-0}" = "1" ] || ! command -v efibootmgr >/dev/null 2>&1; then
  SRC="$(findmnt -no SOURCE "$ESP" 2>/dev/null || echo /dev/sdXN)"
  say "to make MyBoot selectable, register it (adjust disk/part for $SRC):"
  say "  sudo efibootmgr --create --disk /dev/DISK --part N --loader '\\EFI\\MyBoot\\BOOTX64.EFI' --label 'MyBoot' --unicode"
else
  register || true
fi

ok "Done. Reboot and pick 'MyBoot' from the firmware boot menu (F12/F9/Esc)."
say "Your existing bootloader is untouched — keep it as a fallback until MyBoot is proven."

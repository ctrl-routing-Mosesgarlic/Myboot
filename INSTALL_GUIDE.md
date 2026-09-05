# Installing MyBoot (Arch Linux & any UEFI system)

This is how you put MyBoot on a real machine after you've built it — the same idea
as installing GRUB, but with **fewer steps and nothing to regenerate**. It's written
for Arch, with notes for other distros and NixOS.

> **Safety first.** Install MyBoot as an *additional* boot option and keep your
> existing bootloader (GRUB / systemd-boot) in place as a fallback until MyBoot has
> proven itself on your hardware. Have a live USB ready. Nothing here formats a disk
> or deletes another OS's loader.

---

## Why MyBoot is simpler than GRUB (the short version)

MyBoot is a *boot manager*: it **discovers your operating systems at boot time**.
That removes the step every GRUB user re-runs forever.

| | Install command(s) | Config you must generate | After a kernel/OS change |
|---|---|---|---|
| **GRUB** | `grub-install …` **then** `grub-mkconfig -o …` (+ `os-prober`) | `grub.cfg` (generated) | **re-run `grub-mkconfig`** |
| **systemd-boot** | `bootctl install` | one hand-written entry **per OS** | edit entries |
| **MyBoot** | copy `.efi` + one `efibootmgr` line (or `./manage.py install`) | **none** | **nothing — it re-discovers** |

So the whole install is: **put `myboot.efi` on the EFI partition, and tell the
firmware about it.** That's it.

---

## What you need

- A **UEFI** system (check: `[ -d /sys/firmware/efi ] && echo UEFI`).
- The built binary `myboot.efi` (from `cargo build -p myboot --release --target
  x86_64-unknown-uefi`, at `target/x86_64-unknown-uefi/release/myboot.efi`).
- On Arch: `sudo pacman -S efibootmgr dosfstools` (efibootmgr registers the entry;
  dosfstools is only needed if you ever have to create the ESP).
- Root, and the EFI System Partition mounted (usually at `/boot` or `/boot/efi`).

Find your ESP and the disk/partition it lives on:

```sh
lsblk -o NAME,SIZE,FSTYPE,PARTTYPENAME,MOUNTPOINT   # ESP is the vfat "EFI System" one
```

Note its **disk** (e.g. `/dev/nvme0n1`) and **partition number** (e.g. `1`) — you'll
need them for `efibootmgr`.

---

## Option A — one command (recommended)

From the MyBoot repo, in the dev shell (installs the latest GitHub **release**
by default — pass `--repo owner/name` or set `MYBOOT_REPO` until the public repo
is published; `--from build` builds locally instead):

```sh
sudo MYBOOT_REPO=you/myboot ./manage.py install --register
```

It auto-detects the ESP (`/boot/efi` → `/boot` → `/efi`), copies `myboot.efi` to
`<ESP>/EFI/MyBoot/BOOTX64.EFI`, writes a default `config.toml` **only if none
exists** (never clobbers yours), and prints the exact `efibootmgr` command to
register the firmware entry. Add `--register` to have it run that for you, or
`--esp /boot` to point it at a specific mount.

That's the whole install. Skip to **Verify**.

---

## Option B — the manual way (the `grub-install` equivalent)

Works anywhere you just have `myboot.efi` — no repo needed. Two steps:

```sh
# 1. Put MyBoot on the ESP (like grub-install copies GRUB to the ESP)
sudo mkdir -p /boot/EFI/MyBoot
sudo cp myboot.efi /boot/EFI/MyBoot/BOOTX64.EFI      # adjust /boot if your ESP is /boot/efi

# 2. Register it with the firmware (like grub-install's efibootmgr step)
sudo efibootmgr --create \
     --disk /dev/nvme0n1 --part 1 \
     --loader '\EFI\MyBoot\BOOTX64.EFI' \
     --label "MyBoot" --unicode
```

Compare to what GRUB makes you do:

```sh
# GRUB — for reference, NOT needed for MyBoot:
sudo grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=GRUB
sudo grub-mkconfig -o /boot/grub/grub.cfg          # and re-run this on every kernel update
```

MyBoot has **no `grub-mkconfig` step and no `os-prober`** — it finds Windows, your
Arch install, other distros, and NixOS generations on its own, every boot.

---

## Keep your old bootloader as a safety net (important)

`efibootmgr` puts new entries **first** in `BootOrder`. For the first tests you may
prefer your existing bootloader to stay the default and pick MyBoot manually from
the firmware boot menu (F12 / F9 / Esc). To do that, look at the order and move
MyBoot after your current loader:

```sh
efibootmgr                       # note the Boot#### numbers and current BootOrder
# e.g. make "Linux Boot Manager"/GRUB first and MyBoot second:
sudo efibootmgr --bootorder 0001,000A,0002
```

MyBoot also **coexists on disk**: it installs to `\EFI\MyBoot`, so your
`\EFI\systemd`, `\EFI\GRUB`, and `\EFI\Microsoft` loaders are untouched. If MyBoot
ever misbehaves: power-cycle, pick your old loader from the firmware menu, and you're
back. That's your rescue path — keep it until MyBoot has booted your real OSes
several times.

---

## Verify

```sh
efibootmgr                       # you should see a "MyBoot" entry
ls -l /boot/EFI/MyBoot/          # BOOTX64.EFI + config.toml present
```

Reboot, pick **MyBoot** from the firmware boot menu. It should scan your disks and
list your real operating systems (your installed Arch and any others show up by
their vendor directory, e.g. `\EFI\arch`, `\EFI\Microsoft`). Boot one. Done — and
you never have to regenerate anything when you update a kernel.

---

## Making MyBoot the default (only after it's proven)

Once you trust it, put it first:

```sh
sudo efibootmgr                  # find MyBoot's Boot#### (e.g. Boot0007)
sudo efibootmgr --bootorder 0007,0001,0002   # MyBoot first, others after as fallback
```

Optionally set your preferred default OS and timeout in
`/boot/EFI/MyBoot/config.toml` (MyBoot reads it; if it's missing or invalid, MyBoot
just uses safe defaults and still boots).

---

## Uninstall (clean, reversible)

```sh
sudo ./manage.py uninstall        # or, manually:
sudo rm -rf /boot/EFI/MyBoot
sudo efibootmgr -b 0007 -B        # delete MyBoot's entry (use its Boot#### number)
```

Your other loaders were never touched, so removing MyBoot leaves your system exactly
as it was.

---

## Secure Boot

MyBoot itself is not signed for Secure Boot yet. Options: (a) test with Secure Boot
**disabled** in firmware, or (b) enroll MyBoot's hash via MokManager, or (c) chain
through `shim`. For Linux specifically, MyBoot prefers `shimx64.efi` when present, so
the distro's own Secure Boot chain is used for the OS you boot. Signing MyBoot for
Secure Boot is a planned step; until then, disable Secure Boot for testing.

---

## NixOS — do NOT install imperatively

On NixOS, `nixos-rebuild` manages the bootloader and will **revert** anything you
install by hand. Don't run the steps above there. Instead declare MyBoot in your
configuration (the `boot.loader.external` mechanism), so the install is reproducible
and survives rebuilds. (Arch, Fedora, Debian, and the like are fine with the
imperative install above.)

---

## Recovering if a boot ever fails

- **MyBoot starts but nothing boots** → it never dead-ends; it falls through to the
  next healthy OS, and if all fail it returns you to the firmware menu. Pick your old
  loader there.
- **The machine boots straight past MyBoot** → the firmware BootOrder doesn't list it
  first; pick it from the boot menu (F12/F9/Esc) or reorder with `efibootmgr`.
- **Worst case** → boot your live USB, mount the ESP, and either re-copy `myboot.efi`
  or delete `\EFI\MyBoot` and restore your old loader's BootOrder. Nothing MyBoot did
  is destructive.

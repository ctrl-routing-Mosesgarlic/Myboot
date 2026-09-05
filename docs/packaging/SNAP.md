# Packaging MyBoot as a Snap (snapd)

Recipe: `packaging/snap/snapcraft.yaml`. MyBoot needs **classic** confinement (it
writes the ESP and calls `efibootmgr`).

## Build + publish
```sh
sudo snap install snapcraft --classic
snapcraft            # builds myboot_0.1.0_amd64.snap
snapcraft login
snapcraft register myboot
snapcraft upload --release=stable myboot_0.1.0_amd64.snap
```
Classic snaps require a manual review by the Snap Store team — request it after the
first upload.

## User install
```sh
sudo snap install myboot --classic
sudo myboot install --register
```

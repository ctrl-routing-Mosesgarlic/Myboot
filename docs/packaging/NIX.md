# Packaging MyBoot for Nix / NixOS

The repo already has a `flake.nix`. On NixOS you must NOT install a bootloader
imperatively (`nixos-rebuild` reverts it) — declare it instead.

## Expose the package (flake output)
Add a package output so users can `nix profile install github:ctrl-routing-Mosesgarlic/Myboot`:
```nix
# in flake.nix outputs, per system:
packages.myboot = pkgs.rustPlatform.buildRustPackage {
  pname = "myboot"; version = "0.1.0"; src = ./.;
  cargoLock.lockFile = ./Cargo.lock;
  # build the UEFI target + host CLI; install myboot.efi to $out/lib/myboot
};
```

## NixOS: declarative install (the correct way)
Use the external-bootloader mechanism so the install is reproducible and survives
rebuilds:
```nix
boot.loader.external = {
  enable = true;
  installHook = "${pkgs.myboot}/bin/myboot-install";   # copies myboot.efi to the ESP + efibootmgr
};
boot.loader.efi.canTouchEfiVariables = true;
```
Keep systemd-boot/GRUB configured as a fallback until MyBoot is proven on your
hardware.

## Try without installing
```sh
nix run github:ctrl-routing-Mosesgarlic/Myboot -- --help      # once a runnable output is exposed
```

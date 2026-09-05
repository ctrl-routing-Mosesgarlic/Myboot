{
  description = "MyBoot — a transactional cross-OS UEFI boot manager (Rust + asm)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Stable Rust with BOTH the host target (for host tests / host-cli / xtask)
        # and the UEFI target (for the loader itself).
        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [ "x86_64-unknown-uefi" ];
          extensions = [ "rust-src" "clippy" "rustfmt" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rustToolchain
            pkgs.qemu
            pkgs.OVMF
            pkgs.edk2-uefi-shell   # a real EFI app to chainload in QEMU tests
            pkgs.python3
            pkgs.python3Packages.pexpect  # drives QEMU in the pyboot test harness
            pkgs.efibootmgr
            pkgs.xorriso
            pkgs.mtools
            pkgs.dosfstools
            pkgs.parted
            pkgs.gptfdisk
            pkgs.util-linux
          ];

          shellHook = ''
            echo "MyBoot dev shell — tooling: ./manage.py --help"
            echo "  $(rustc --version)"
            echo "  $(qemu-system-x86_64 --version | head -n1)"

            mkdir -p ./firmware
            if [ ! -f ./firmware/OVMF_VARS.fd ]; then
              cp ${pkgs.OVMF.fd}/FV/OVMF_VARS.fd ./firmware/OVMF_VARS.fd
              chmod +w ./firmware/OVMF_VARS.fd
            fi
            export OVMF_CODE="${pkgs.OVMF.fd}/FV/OVMF_CODE.fd"
            export OVMF_VARS="$(pwd)/firmware/OVMF_VARS.fd"
            # A real, different EFI application for `./manage.py smoke` to chainload as a
            # true end-to-end boot test (the UEFI shell appears on handoff).
            for cand in ${pkgs.edk2-uefi-shell}/shell.efi ${pkgs.edk2-uefi-shell}/*.efi; do
              [ -f "$cand" ] && export MYBOOT_UEFI_SHELL="$cand" && break
            done
            echo "  OVMF_CODE=$OVMF_CODE"
            echo "  OVMF_VARS=$OVMF_VARS"
            [ -n "''${MYBOOT_UEFI_SHELL:-}" ] && echo "  MYBOOT_UEFI_SHELL=$MYBOOT_UEFI_SHELL"
            echo ""
            echo "  cargo test  (per pure crate)          → ./scripts/run-tests.sh"
            echo "  build the loader for UEFI             → cargo build -p myboot --release --target x86_64-unknown-uefi"
            echo "  run in QEMU with a synthetic ESP      → ./manage.py smoke"
          '';
        };
      });
}

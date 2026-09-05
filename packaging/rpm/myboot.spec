Name:           myboot
Version:        0.1.0
Release:        1%{?dist}
Summary:        Intelligent, transactional UEFI boot manager (auto-discovers OSes)
License:        MIT OR Apache-2.0
URL:            https://github.com/ctrl-routing-Mosesgarlic/Myboot
Source0:        %{url}/archive/refs/tags/v%{version}.tar.gz
BuildRequires:  cargo, rust
Requires:       efibootmgr
Recommends:     dosfstools
ExclusiveArch:  x86_64

%description
MyBoot discovers the operating systems on every disk at boot time (no config to
generate), presents a menu, and chainloads the chosen OS with a transactional,
roll-back-safe lifecycle.

%prep
%autosetup -n Myboot-%{version}

%build
rustup target add x86_64-unknown-uefi || true
cargo build -p myboot --release --target x86_64-unknown-uefi
cargo build --manifest-path host-cli/Cargo.toml --release

%install
install -Dm644 target/x86_64-unknown-uefi/release/myboot.efi %{buildroot}%{_libdir}/myboot/myboot.efi
install -Dm755 host-cli/target/release/myboot %{buildroot}%{_bindir}/myboot

%files
%license LICENSE-MIT LICENSE-APACHE
%{_bindir}/myboot
%{_libdir}/myboot/myboot.efi

%changelog
* Fri Sep 05 2026 Moses <ctrl-routing-Mosesgarlic@users.noreply.github.com> - 0.1.0-1
- Initial package

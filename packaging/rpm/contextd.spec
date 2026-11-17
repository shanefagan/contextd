# Ship one package; newer rpmbuild otherwise also emits a -debuginfo package.
%global debug_package %{nil}

Name:            contextd
Version:         0.10.0
Release:         1%{?dist}
Summary:         Lightweight game and hardware context daemon (Portable Hybrid)
License:         MIT
URL:             https://github.com/shanefagan/contextd


Source0:         %{name}-%{version}.tar.gz

BuildRequires:   cargo, systemd-devel, git
Requires:        systemd, systemd-container

%description
A generic, lightweight Linux daemon that exposes process context (e.g., gaming activity)
and hardware inventory via a Varlink interface. Packaged as a Portable Hybrid Service,
with an optional OpenRGB SDK bridge.

%prep
%autosetup

%build
# Use fixed flags for bundled sqlite linking
unset CFLAGS CXXFLAGS LDFLAGS RUSTFLAGS
cargo build --release --workspace --locked

%install
./scripts/package-portable.sh build/tree target/release
mkdir -p %{buildroot}/opt
cp -a build/tree/contextd %{buildroot}/opt/contextd
cp -a build/tree/host/usr %{buildroot}/usr

%post
# The control socket's group must exist before its socket unit starts.
systemd-sysusers contextd.conf
systemctl daemon-reload || :
echo ">>> Attaching contextd portable service from /opt..."
portablectl detach --now --enable contextd 2>/dev/null || :
portablectl attach --now --enable --copy=symlink --profile=trusted /opt/contextd

%preun
if [ $1 -eq 0 ]; then
    systemctl disable --now openrgb-contextd-bridge.service 2>/dev/null || :
    echo ">>> Detaching contextd portable service..."
    portablectl detach --now --enable contextd || :
fi

%files
/opt/contextd/
%{_bindir}/openrgb-contextd-bridge
%{_bindir}/contextctl
/usr/lib/systemd/system/openrgb-contextd-bridge.service
/usr/lib/sysusers.d/contextd.conf

%changelog
* Tue Nov 17 2026 Shane Fagan <shane@performativenonsense.com> - 0.10.0-1
- Socket activation, rgb control group, OpenRGB bridge packaged
* Mon Apr 13 2026 Shane Fagan <shane@performativenonsense.com> - 0.1.0-1
- Initial production packaging with /opt based portable infrastructure

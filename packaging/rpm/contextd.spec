Name:            contextd
Version:         0.1.0
Release:         1%{?dist}
Summary:         Lightweight game and hardware context daemon (Portable Hybrid)
License:         MIT or Apache-2.0
URL:             https://github.com/shanefagan/contextd
Source0:         %{name}-%{version}.tar.gz

BuildRequires:   cargo, systemd-devel, git
Requires:        systemd, portablectl

%description
A generic, lightweight Linux daemon that exposes process context (e.g., gaming activity) 
and hardware inventory via a Varlink interface. Packaged as a Portable Hybrid Service.

%prep
# In a real CI environment, we would use %autosetup
# For now we assume we are building from the checked-out source

%build
# Use fixed flags for bundled sqlite linking
unset CFLAGS CXXFLAGS LDFLAGS RUSTFLAGS
cargo build --release

%install
# Assemble the Portable Root in /opt/contextd
mkdir -p %{buildroot}/opt/contextd/usr/bin
mkdir -p %{buildroot}/opt/contextd/usr/lib/systemd/system
mkdir -p %{buildroot}/opt/contextd/etc
mkdir -p %{buildroot}/opt/contextd/home
mkdir -p %{buildroot}/opt/contextd/sys
mkdir -p %{buildroot}/opt/contextd/run
mkdir -p %{buildroot}/opt/contextd/proc

cp target/release/contextd %{buildroot}/opt/contextd/usr/bin/
cp packaging/contextd.service %{buildroot}/opt/contextd/usr/lib/systemd/system/

# Create os-release
cat <<EOF > %{buildroot}/opt/contextd/etc/os-release
ID=contextd
NAME="Context Daemon Portable Root"
PRETTY_NAME="Context Daemon Portable Root"
EOF

%post
echo ">>> Attaching contextd portable service from /opt..."
portablectl attach --now --copy=symlink --profile=trusted /opt/contextd

%preun
if [ $1 -eq 0 ]; then
    echo ">>> Detaching contextd portable service..."
    portablectl detach contextd --now || true
fi

%postun
if [ $1 -ge 1 ]; then
    echo ">>> Reloading contextd portable service from /opt..."
    portablectl detach contextd --now || true
    portablectl attach --now --copy=symlink --profile=trusted /opt/contextd
fi

%files
/opt/contextd/

%changelog
* Mon Apr 13 2026 Shane Fagan <mail@shanefagan.com> - 0.1.0-1
- Initial production packaging with /opt based portable infrastructure

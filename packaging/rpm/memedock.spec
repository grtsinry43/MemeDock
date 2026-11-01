
Name: memedock
Version: %{version}
Release: 1%{?dist}
Summary: Unified sticker manager for easy importing and quick sharing
License: MIT
URL: https://github.com/grtsinry43/MemeDock
BuildArch: x86_64
Requires: gtk4 >= 4.22
Requires: libadwaita >= 1.9

%description
MemeDock organizes stickers in one place, imports them quickly, and makes
sharing the right image in a chat effortless.

%install
mkdir -p %{buildroot}
cp -a %{stage}/usr %{buildroot}/

%files
%{_bindir}/memedock
%{_datadir}/applications/com.grtsinry43.memedock.desktop
%{_datadir}/icons/hicolor/scalable/apps/com.grtsinry43.memedock.svg
%{_licensedir}/memedock/*

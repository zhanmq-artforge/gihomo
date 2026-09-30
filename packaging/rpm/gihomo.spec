Name:           gihomo
Version:        1.0.0
Release:        1%{?dist}
Summary:        Modern native GTK4 + Libadwaita management client for Mihomo

License:        GPL-3.0-or-later
URL:            https://github.com/artforge/gihomo
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.75
BuildRequires:  cargo
BuildRequires:  pkgconfig(gtk4) >= 4.12
BuildRequires:  pkgconfig(libadwaita-1) >= 1.5
BuildRequires:  pkgconfig(glib-2.0)
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

Requires:       gtk4 >= 4.12
Requires:       libadwaita >= 1.5
Requires:       glib2
Requires:       libcap

%description
Gihomo is a modern native desktop client for Mihomo (Clash.Meta)
running directly on Linux and GNOME environments.
Features subscription management, node latency testing, GNOME system proxy
integration, kernel TUN mode toggle, and real-time network traffic monitoring.

%prep
%autosetup

%build
cargo build --release

%install
rm -rf %{buildroot}
install -D -p -m 0755 target/release/gihomo %{buildroot}%{_bindir}/gihomo
install -D -p -m 0755 assets/mihomo %{buildroot}%{_libdir}/gihomo/bin/mihomo
install -D -p -m 0644 data/art.artforge.Gihomo.desktop.in %{buildroot}%{_datadir}/applications/art.artforge.Gihomo.desktop
install -D -p -m 0644 data/icons/hicolor/scalable/apps/art.artforge.Gihomo.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/art.artforge.Gihomo.svg
install -D -p -m 0644 data/icons/hicolor/scalable/apps/art.artforge.Gihomo-blue.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/art.artforge.Gihomo-blue.svg
install -D -p -m 0644 data/icons/hicolor/scalable/apps/art.artforge.Gihomo-gray.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/art.artforge.Gihomo-gray.svg
install -D -p -m 0644 data/icons/hicolor/scalable/apps/art.artforge.Gihomo-green.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/art.artforge.Gihomo-green.svg
install -D -p -m 0644 data/icons/hicolor/scalable/apps/art.artforge.Gihomo-red.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/art.artforge.Gihomo-red.svg
install -D -p -m 0644 data/art.artforge.Gihomo.gschema.xml %{buildroot}%{_datadir}/glib-2.0/schemas/art.artforge.Gihomo.gschema.xml
install -D -p -m 0644 data/art.artforge.Gihomo.metainfo.xml.in %{buildroot}%{_datadir}/metainfo/art.artforge.Gihomo.metainfo.xml
install -D -p -m 0644 data/art.artforge.Gihomo.rules %{buildroot}%{_datadir}/polkit-1/rules.d/art.artforge.Gihomo.rules

%post
if [ $1 -eq 1 ]; then
    glib-compile-schemas %{_datadir}/glib-2.0/schemas &>/dev/null || :
    update-desktop-database &>/dev/null || :
    touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
    gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :
    if [ -x %{_libdir}/gihomo/bin/mihomo ]; then
        setcap cap_net_admin,cap_net_bind_service=+ep %{_libdir}/gihomo/bin/mihomo &>/dev/null || :
    elif [ -x %{_bindir}/mihomo ]; then
        setcap cap_net_admin,cap_net_bind_service=+ep %{_bindir}/mihomo &>/dev/null || :
    fi
fi

%postun
glib-compile-schemas %{_datadir}/glib-2.0/schemas &>/dev/null || :
update-desktop-database &>/dev/null || :
touch --no-create %{_datadir}/icons/hicolor &>/dev/null || :
gtk-update-icon-cache %{_datadir}/icons/hicolor &>/dev/null || :

%files
%license LICENSE*
%{_bindir}/gihomo
%{_libdir}/gihomo/bin/mihomo
%{_datadir}/applications/art.artforge.Gihomo.desktop
%{_datadir}/icons/hicolor/scalable/apps/art.artforge.Gihomo*.svg
%{_datadir}/glib-2.0/schemas/art.artforge.Gihomo.gschema.xml
%{_datadir}/metainfo/art.artforge.Gihomo.metainfo.xml
%{_datadir}/polkit-1/rules.d/art.artforge.Gihomo.rules

%changelog
* Tue Sep 29 2026 ArtForge Team <team@artforge.org> - 0.4.3-1
- Add subscription editing and sequential bulk refresh with per-item failure summary
- Refine subscription management action hierarchy

* Tue Sep 29 2026 ArtForge Team <team@artforge.org> - 0.4.2-1
- Bind the Mihomo controller to loopback and persist a private random secret
- Isolate system proxy settings and preference writes from the GTK main loop
- Tighten application service boundaries and use typed application errors

* Sun Sep 27 2026 ArtForge Team <team@artforge.org> - 0.3.0-1
- Native D-Bus StatusNotifierItem (SNI) system tray integration (ksni)
- Comprehensive tray context menu: Open Dashboard, System Proxy toggle, TUN toggle, Proxy Mode switch, Quit
- Close-to-tray background daemon behavior with GLib hold guard
- Autostart at system login configuration with --minimized launch argument
- General settings tab for background behavior customization

* Sun Sep 27 2026 ArtForge Team <team@artforge.org> - 0.2.0-1
- Ubuntu Settings / Libadwaita adaptive split view navigation (AdwNavigationSplitView)
- Interactive loading spinners and sensitive feedback across kernel, proxies, and subscriptions
- Zero clippy warnings, complete bilingual localization hot-reload

* Sun Sep 27 2026 ArtForge Team <team@artforge.org> - 0.1.0-1
- Initial release of Gihomo v0.1.0 (Native Mihomo management client)

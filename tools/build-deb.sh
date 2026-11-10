#!/bin/sh
# Bygger dist/krabba-spelet_<version>_amd64.deb
set -e
cd "$(dirname "$0")/.."
V=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
cargo build --release --bin krabba_spelet
R=dist/deb/krabba-spelet_${V}_amd64
rm -rf "$R" && mkdir -p "$R/DEBIAN" "$R/usr/games" "$R/usr/share/applications" "$R/usr/share/doc/krabba-spelet"
install -m755 target/release/krabba_spelet "$R/usr/games/krabba-spelet"
strip "$R/usr/games/krabba-spelet" 2>/dev/null || true
install -m644 assets/krabba-spelet.desktop "$R/usr/share/applications/"
for s in 32 48 64 128 256; do
  mkdir -p "$R/usr/share/icons/hicolor/${s}x${s}/apps"
  install -m644 assets/icon/krabba-$s.png "$R/usr/share/icons/hicolor/${s}x${s}/apps/krabba-spelet.png"
done
install -m644 README.md "$R/usr/share/doc/krabba-spelet/"
install -m644 assets/fonts/OFL-*.txt "$R/usr/share/doc/krabba-spelet/"
SIZE=$(du -sk "$R/usr" | cut -f1)
cat > "$R/DEBIAN/control" <<CTL
Package: krabba-spelet
Version: $V
Section: games
Priority: optional
Architecture: amd64
Depends: libc6 (>= 2.35), libgcc-s1, libx11-6, libxcursor1, libxkbcommon0
Recommends: libwayland-client0, xclip | wl-clipboard
Installed-Size: $SIZE
Maintainer: felixarvid2 <felixarvid2@users.noreply.github.com>
Homepage: https://github.com/felixarvid2/krabba-spelet-rust
Description: Nätverksspel där du felsöker Cisco-utrustning via CLI
 Krabba spelet: du är nätverkstekniker på Nordvik och felsöker veckans fel
 i en simulerad Cisco-miljö (VLAN, STP, routing, OSPF, BGP, NAT, DHCP,
 ACL, IPsec m.m.) i en 2D-pixelvärld. Kurs 1 vecka 1–10, fem byggen från
 grunden och kurs 2 vecka 11–14.
CTL
cat > "$R/DEBIAN/postinst" <<'PI'
#!/bin/sh
set -e
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q /usr/share/icons/hicolor || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q /usr/share/applications || true
PI
chmod 755 "$R/DEBIAN/postinst"
dpkg-deb --root-owner-group --build "$R" "dist/krabba-spelet_${V}_amd64.deb"
echo "Klart: dist/krabba-spelet_${V}_amd64.deb"

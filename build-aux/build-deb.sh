#!/usr/bin/env bash
#
# Builds a Debian package.
#
# The fleet's CLIs are packaged by cargo-deb, which lists the files it ships in
# Cargo.toml. That does not fit here: an installed Comma is a binary plus a
# resource bundle, a desktop entry, AppStream metadata, a GSettings schema, two
# icons, and a translation catalogue for every language, and Meson already knows
# where every one of them goes. So this stages a normal `meson install` into a
# directory and wraps that directory, rather than restating the install list in
# a second place where the two could come to disagree.
#
# Nothing here compiles the schema, updates the icon cache, or rebuilds the
# desktop database. Those are dpkg triggers on packages this one depends on, and
# they run for every package that installs into the directories they watch.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

set -euo pipefail

source_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
revision=${DEB_REVISION:-1}

version=$(
    sed -n "s/^  version: '\([^']*\)',$/\1/p" "$source_root/meson.build"
)
if [ -z "$version" ]; then
    echo "build-deb.sh: could not read the version out of meson.build" >&2
    exit 1
fi

architecture=$(dpkg --print-architecture)
staging=$(mktemp -d)
build=$(mktemp -d)
trap 'rm -rf "$staging" "$build"' EXIT

# Meson writes the install prefix into src/config.rs, and this build needs it to
# say /usr while a working copy needs it to say whatever that copy was set up
# with. The file is a build artifact rather than source, so it is put back
# afterwards instead of being left pointing at a prefix nothing is installed to.
config=$source_root/src/config.rs
saved=$build/config.rs.saved
if [ -f "$config" ]; then
    cp "$config" "$saved"
fi
restore() {
    rm -rf "$staging"
    if [ -f "$saved" ]; then
        cp "$saved" "$config"
    fi
    rm -rf "$build"
}
trap restore EXIT

meson setup "$build/meson" "$source_root" --prefix=/usr --buildtype=release
meson install -C "$build/meson" --destdir "$staging"

# mktemp makes a directory only its owner can enter, and dpkg-deb records the
# staging root as the package's own `./`, so leaving it alone ships a package
# whose top directory nobody can read.
chmod 0755 "$staging"

# Debian expects the licence at a fixed path, which is not something Meson has
# an opinion about.
install -D -m 0644 "$source_root/LICENSE" "$staging/usr/share/doc/comma/copyright"
install -D -m 0644 "$source_root/README.md" "$staging/usr/share/doc/comma/README.md"

# dpkg-shlibdeps reads the libraries the binary was linked against, which is the
# right answer for everything except GTK and libadwaita: the bindings are
# compiled against a stated version, and asking the binary produces a lower floor
# than the one the code was written to. Those two are named at the version this
# was built and tested against; the rest is whatever the linker actually needs.
mkdir -p "$staging/debian"
printf 'Source: comma\n\nPackage: comma\nArchitecture: %s\n' "$architecture" \
    > "$staging/debian/control"
depends=$(
    cd "$staging" &&
        dpkg-shlibdeps -O usr/bin/comma 2>/dev/null |
        sed 's/^shlibs:Depends=//'
)
rm -rf "$staging/debian"

depends=$(sed 's/libgtk-4-1 ([^)]*)/libgtk-4-1 (>= 4.18)/' <<< "$depends")
depends=$(sed 's/libadwaita-1-0 ([^)]*)/libadwaita-1-0 (>= 1.7)/' <<< "$depends")
depends="$depends, dconf-gsettings-backend | gsettings-backend"

mkdir -p "$staging/DEBIAN"
cat > "$staging/DEBIAN/control" <<CONTROL
Package: comma
Version: $version-$revision
Section: gnome
Priority: optional
Architecture: $architecture
Depends: $depends
Maintainer: David M. Anderson <david.anderson@excelano.com>
Homepage: https://github.com/excelano/comma
Description: Edit delimited text files
 Comma is a small GNOME editor for CSV, TSV, and other delimited text files.
 It shows a file as a grid of values and saves it back as the same kind of
 file it opened, leaving every row you did not edit byte for byte as it was.
 .
 Values are treated as text and never reinterpreted, so leading zeros
 survive, long identifiers stay whole, and dates are whatever the file says
 they are. Comma is not a spreadsheet: there are no formulas, no cell
 formatting, and no second file format to save into.
CONTROL

output=$source_root/target/debian
mkdir -p "$output"
package=$output/comma_${version}-${revision}_${architecture}.deb
dpkg-deb --root-owner-group --build "$staging" "$package" > /dev/null

echo "$package"

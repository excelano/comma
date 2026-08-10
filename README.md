# Comma

A small GNOME editor for CSV, TSV, and other delimited text files. Comma shows a file as a grid of values and saves it back as the same kind of file it opened, leaving every row you did not edit byte for byte as it was.

Values are treated as text and never reinterpreted, so leading zeros survive, long identifiers stay whole, and dates are whatever the file says they are. Comma is not a spreadsheet: there are no formulas, no cell formatting, and no second file format to save into. If you need those, Gnumeric and LibreOffice Calc are excellent and Comma is not trying to replace them.

Comma is a sibling to [Apostrophe](https://gitlab.gnome.org/World/apostrophe). Both are named for punctuation, and both edit files whose structure is invisible skeleton rather than content.

## Status

Released and in daily use, at 0.1.1. Comma opens a file from the command line, the file manager, or its own dialog; guesses the delimiter, shows you which one it guessed, and lets you correct it in one click; and can be told that the first row is column titles rather than data. Cells are editable, rows and columns can be inserted and deleted, everything can be undone and redone, and saving a file you changed in one place changes that one line and nothing else.

Clicking a column header puts the grid in that order without touching the file; one menu item writes that order down when you want it. Searching hides the rows nothing matched in, so what you are looking at before you replace is exactly what you are about to change.

Export writes what the grid is showing as a PDF, a web page, or an OpenDocument spreadsheet. All three are output: they are never reopened, never offered as Save, and every value goes into them as text, so a leading zero is still a leading zero and a sixteen-digit account number is still itself.

The keyboard reaches everything, the row numbers and column headings are handles for the operations that act on them, and Comma speaks English and German. Still to come: the row-number gutter scrolls away horizontally instead of staying pinned, and nothing acts on more than one cell at a time.

## Installing

Comma is built against GTK 4.18 and libadwaita 1.7, so it needs a distribution carrying both — Debian 13 or newer, or Ubuntu 25.04 or newer. Older releases are not a matter of rebuilding; the widgets are not there.

### Debian and Ubuntu

Comma is packaged for amd64 and arm64 in the Excelano apt repository. Add the repository once:

```sh
curl -fsSL https://excelano.com/apt/setup.sh | sudo sh
```

Then:

```sh
sudo apt install comma
```

Updates arrive with `apt upgrade` like any other package.

### From source

Comma builds with Meson, which drives Cargo for the Rust compilation and handles the desktop entry, AppStream metadata, GSettings schema, icons, and translations. Build dependencies on Debian 13:

```sh
sudo apt install meson ninja-build libgtk-4-dev libadwaita-1-dev \
    blueprint-compiler gettext libglib2.0-dev-bin desktop-file-utils appstream
```

The Rust toolchain has to come from [rustup](https://rustup.rs) rather than from apt: the GTK bindings ask for Rust 1.92 and Debian 13 packages 1.85.

Then configure, build, and install into your home prefix:

```sh
meson setup builddir --prefix="$HOME/.local"
ninja -C builddir install
```

Run it with `~/.local/bin/comma`. Installing is not optional — Comma loads its compiled resource bundle and its GSettings schema from the install prefix, so running the binary straight out of the build directory will not work. If you have also installed the package, note that `~/.local/bin` usually comes first on `PATH`, so this build is the one that runs.

## Development

`ninja -C builddir test` validates the desktop entry, the AppStream metainfo, and the GSettings schema; `cargo test` runs the rest.

Meson generates `src/config.rs` during configuration, so `cargo build` on its own only works once `meson setup` has run at least once. Configuring also checks that `meson.build` and `Cargo.toml` agree about the version, because they each carry their own copy of it.

`build-aux/build-deb.sh` builds the Debian package, by staging a normal `meson install` and wrapping it. It configures with `--prefix=/usr`, so it rewrites `src/config.rs` and puts the previous one back when it finishes.

## License

MIT. See [LICENSE](LICENSE).

Built with the assistance of Claude (Anthropic).

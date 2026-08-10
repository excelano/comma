# Comma

A small GNOME editor for CSV, TSV, and other delimited text files. Comma shows a file as a grid of values and saves it back as the same kind of file it opened, leaving every row you did not edit byte for byte as it was.

Values are treated as text and never reinterpreted, so leading zeros survive, long identifiers stay whole, and dates are whatever the file says they are. Comma is not a spreadsheet: there are no formulas, no cell formatting, and no second file format to save into. If you need those, Gnumeric and LibreOffice Calc are excellent and Comma is not trying to replace them.

Comma is a sibling to [Apostrophe](https://gitlab.gnome.org/World/apostrophe). Both are named for punctuation, and both edit files whose structure is invisible skeleton rather than content.

## Status

Early, and useful. Comma opens a file from the command line, the file manager, or its own dialog; guesses the delimiter, shows you which one it guessed, and lets you correct it in one click; and can be told that the first row is column titles rather than data. Cells are editable, edits can be undone and redone, and saving a file you changed in one place changes that one line and nothing else.

Not written yet: inserting and deleting rows and columns, sorting, filtering, find and replace, and the PDF, HTML, and ODS exports. Keyboard navigation works but has not been gone over properly, the icon is a placeholder, and there are no translations yet.

## Building

Comma builds with Meson, which drives Cargo for the Rust compilation and handles the desktop entry, AppStream metadata, GSettings schema, icons, and translations. Build dependencies on Debian 13:

```sh
sudo apt install meson ninja-build libgtk-4-dev libadwaita-1-dev \
    blueprint-compiler gettext libglib2.0-dev-bin desktop-file-utils appstream
```

Then configure, build, and install into your home prefix:

```sh
meson setup builddir --prefix="$HOME/.local"
ninja -C builddir install
```

Run it with `~/.local/bin/comma`. Installing is not optional — Comma loads its compiled resource bundle and its GSettings schema from the install prefix, so running the binary straight out of the build directory will not work.

`ninja -C builddir test` validates the desktop entry, the AppStream metainfo, and the GSettings schema.

Meson generates `src/config.rs` during configuration, so `cargo build` on its own only works once `meson setup` has run at least once.

## License

MIT. See [LICENSE](LICENSE).

Built with the assistance of Claude (Anthropic).

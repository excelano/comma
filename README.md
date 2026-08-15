# Comma

A small GNOME editor for CSV, TSV, and other delimited text files. Comma shows a file as a grid of values and saves it back as the same kind of file it opened, leaving every row you did not edit byte for byte as it was.

Values are treated as text and never reinterpreted, so leading zeros survive, long identifiers stay whole, and dates are whatever the file says they are. Comma is not a spreadsheet: there are no formulas, no cell formatting, and no second file format to save into. If you need those, Gnumeric and LibreOffice Calc are excellent and Comma is not trying to replace them.

Comma took its inspiration from [Apostrophe](https://gitlab.gnome.org/World/apostrophe). Both are named for punctuation, and both edit files whose structure is invisible skeleton rather than content.

## Status

Released and in daily use. Comma opens a file from the command line, the file manager, or its own dialog, and guesses the two things a delimited file does not say about itself: which delimiter separates its fields, and whether its first row is column titles or data. Both guesses are shown under the header bar button that says how the file is being read, and both are one click from being corrected. Cells are editable, rows and columns can be inserted and deleted, everything can be undone and redone, and saving a file you changed in one place changes that one line and nothing else.

Clicking a column header puts the grid in that order without touching the file; one menu item writes that order down when you want it. Searching hides the rows nothing matched in, so what you are looking at before you replace is exactly what you are about to change.

A column can also be held to a condition of its own. Right-click its heading, or press Ctrl+Shift+F on a cell in it, and pick from nine: contains and does not contain, is and is not, is empty and is not empty, is greater than, is less than, and is between. Several columns can each carry one and they narrow one another, while the search bar asks its question of the whole row, so the two do different jobs and work together. Right-clicking a cell offers the shortest path of all — hold this column to the value already written here. Numbers are read as numbers, so a price above 100 turns up neither a blank cell nor "£5", and the order text goes in is your own language's rather than the ASCII table's. Every condition in force is spelled out on a bar under the toolbar with the button that removes it, and the title says how many rows of how many are showing, because a view that quietly hides rows is a lie about the file. Filtering never writes: unlike a sort, there is no menu item that commits it, and there never will be, since a file cannot record which of its rows you were not looking at.

Export writes what the grid is showing as a PDF, a web page, or an OpenDocument spreadsheet. All three are output: they are never reopened, never offered as Save, and every value goes into them as text, so a leading zero is still a leading zero and a sixteen-digit account number is still itself.

The keyboard reaches everything, the row numbers and column headings are handles for the operations that act on them, and both stay put as you scroll: the headings across the top, the numbers down the side. Comma speaks English and German. Still to come: nothing acts on more than one cell at a time.

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
meson setup builddir --prefix="$HOME/.local" --buildtype=debugoptimized
ninja -C builddir install
```

Ask for `debugoptimized` rather than taking Meson's default, which is an unoptimised build. Comma spends the time between launching and drawing a file building a cell widget for every column of every row it has realised, and unoptimised that costs a wide file roughly a further fifth of a second. `debugoptimized` keeps enough debug information for a panic to name the line it came from. The Debian package is built as `release`, so this only affects installing from source.

Run it with `~/.local/bin/comma`. Installing is not optional — Comma loads its compiled resource bundle and its GSettings schema from the install prefix, so running the binary straight out of the build directory will not work. If you have also installed the package, note that `~/.local/bin` usually comes first on `PATH`, so this build is the one that runs.

## Development

`ninja -C builddir test` validates the desktop entry, the AppStream metainfo, and the GSettings schema; `cargo test` runs the rest.

Meson generates `src/config.rs` during configuration, so `cargo build` on its own only works once `meson setup` has run at least once. Configuring also checks that `meson.build` and `Cargo.toml` agree about the version, because they each carry their own copy of it.

`build-aux/build-deb.sh` builds the Debian package, by staging a normal `meson install` and wrapping it. It configures with `--prefix=/usr`, so it rewrites `src/config.rs` and puts the previous one back when it finishes.

## License

MIT. See [LICENSE](LICENSE).

Built with the assistance of Claude (Anthropic).

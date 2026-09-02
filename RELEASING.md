# Releasing Comma

The release loop lives in `~/notes/releasing.md` — the ordered steps, the apt
step, the spent-tag rule, and the standing facts about tokens and secrets.
Failure recipes are in `~/notes/build_release_gotchas.md`. This file carries
what is true of Comma and not of its siblings.

| | |
|---|---|
| Loop | hand-cut |
| Version lives in | `meson.build`, `Cargo.toml`, `Cargo.lock`, metainfo |
| Version that reaches users | `meson.build` |
| `apt-ship` argument | `comma` |
| Packages per release | two, amd64 and arm64 |
| Channels | apt, and Flathub once it is accepted |

**Comma is a desktop application**, which is what makes its loop the hand-cut
one. apt and Flathub are both channels for it, and neither replaces the other:
apt is what this machine and any Debian 13 or Ubuntu 25.04 box installs from,
and Flathub is what reaches everything older than that, since a Flatpak carries
its own GTK and so is not held to the 4.18 floor.

**The version lives in four places and only one of them reaches users.**
`meson.build` is authoritative: `build-aux/build-deb.sh` reads the version
straight out of it with a `sed`, and `meson.project_version()` becomes `VERSION`
in the generated `src/config.rs`, which is what the About dialog shows. The
`version` in `Cargo.toml` and its matching line in `Cargo.lock` are kept in step
so nothing disagrees, not because anything ships them — a build refreshes the
lock. The fourth is the metainfo release entry below.

**The metainfo carries a `<release>` entry per version**, in
`data/com.excelano.Comma.metainfo.xml.in`, and the software centres read the
history out of it. Add the new entry with its date and a description of what
changed, in the same voice as the release notes rather than a changelog line.

**Those release-note paragraphs are extracted for translation.** This is the
step that looks finished and is not: a bump with no `po` work leaves the release
notes untranslated, and nothing fails to tell you. After editing the metainfo:

```sh
ninja -C builddir comma-pot comma-update-po
```

then write the German and confirm the count is clean:

```sh
msgfmt --statistics -o /dev/null po/de.po
```

Nothing fuzzy, nothing untranslated. All of it belongs in the release commit,
alongside the version bump.

**Publishing the release is what starts the packaging.** The tag triggers
nothing here; `deb.yml` runs on `release: published`, which fires because you
created the release rather than a workflow. Title it as a product release,
`Comma 1.2.3`, rather than as the bare tag the CLIs use.

**Confirm both packages are attached before `apt-ship`.** `deb.yml` builds
amd64 on `ubuntu-latest` and arm64 on `ubuntu-24.04-arm`, each in a
`rust:1-trixie` container, and checks that the package labelled for an
architecture really holds a binary of it — a cross-compiled mislabel installs
and then does not run. A release that ships one package still looks complete on
the release page.

**Your own copy does not come from apt.** Comma is installed here from the
source tree, `ninja -C builddir install`, into `~/.local/bin`, so shipping to
apt does not refresh what you run. Install locally as well, or the version you
are using stays behind the one you released.

**Flathub is a second publish, and the tag does not trigger it.** The manifest
Flathub builds lives in `flathub/com.excelano.Comma`, not here; the copy in
`build-aux/flatpak/` is the development one and differs in a single module
source, a directory where the published one names a tag and a commit. After the
GitHub release is out, edit that repository's manifest to the new tag and its
commit and push. The buildbot does the rest, and the build lands in Flathub's
test repository first, where it can be installed and looked at before it is
published.

**What breaks this is `cargo-sources.json` going stale.** Flathub builds with no
network, so every crate is listed with its checksum in
`build-aux/flatpak/cargo-sources.json`, and a build whose `Cargo.lock` has moved
past that file fails at a download it is not allowed to make. The `flatpak`
workflow regenerates it on any branch that moves `Cargo.lock`, which covers the
weekly Dependabot bump; the case it does not cover is a lock edited by hand and
merged without CI. Check it before tagging:

```sh
python3 flatpak-cargo-generator.py Cargo.lock -o build-aux/flatpak/cargo-sources.json
git diff --stat build-aux/flatpak/cargo-sources.json
```

**Build it before submitting anything.** The runtimes are heavy but they are
shared and they stay installed:

```sh
flatpak-builder --user --install --force-clean \
    .flatpak-builder/build build-aux/flatpak/com.excelano.Comma.yaml
```

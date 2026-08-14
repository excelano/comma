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
| Channels | apt only |

**Comma is a desktop application**, which is what makes its loop the hand-cut
one, and apt is the whole of its distribution. Flatpak is deferred rather than
ruled out; when it arrives it is another channel beside this one rather than a
replacement for it.

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

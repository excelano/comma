project(
  'comma',
  'rust',
  version: '0.1.4',
  license: 'MIT',
  meson_version: '>= 1.0.0',
)

i18n = import('i18n')
gnome = import('gnome')

# The version is written twice: here, which is where the About dialog, the
# AppStream metadata and the Debian package all read it from, and in Cargo.toml,
# which is where anyone looking at a Rust project would look first. Two places
# drift, so configuring says so rather than letting a release go out disagreeing
# with itself.
cargo_version = run_command(
  'sed',
  '-n',
  's/^version = "\\(.*\\)"$/\\1/p',
  meson.project_source_root() / 'Cargo.toml',
  check: true,
).stdout().strip()

if cargo_version != meson.project_version()
  error(
    'meson.build says version @0@ and Cargo.toml says @1@'.format(
      meson.project_version(),
      cargo_version,
    ),
  )
endif

application_id = 'com.excelano.Comma'
gettext_package = meson.project_name()

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
datadir = prefix / get_option('datadir')
localedir = prefix / get_option('localedir')
pkgdatadir = datadir / meson.project_name()

subdir('data')
subdir('src')
subdir('po')

gnome.post_install(
  glib_compile_schemas: true,
  gtk_update_icon_cache: true,
  update_desktop_database: true,
)

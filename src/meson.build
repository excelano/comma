blueprint_compiler = find_program('blueprint-compiler')

blueprints = custom_target(
  'blueprints',
  input: files('gtk/window.blp'),
  output: '.',
  command: [
    blueprint_compiler,
    'batch-compile',
    '@OUTPUT@',
    '@CURRENT_SOURCE_DIR@',
    '@INPUT@',
  ],
)

resources = gnome.compile_resources(
  meson.project_name(),
  'comma.gresource.xml',
  dependencies: blueprints,
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
)

conf = configuration_data()
conf.set_quoted('APP_ID', application_id)
conf.set_quoted('VERSION', meson.project_version())
conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
conf.set_quoted('LOCALEDIR', localedir)
conf.set_quoted('PKGDATADIR', pkgdatadir)

configure_file(input: 'config.rs.in', output: 'config.rs', configuration: conf)

# Cargo compiles from the source tree, so the generated config has to be copied
# back into it. src/config.rs is a build artifact and is gitignored.
run_command(
  'cp',
  meson.project_build_root() / 'src' / 'config.rs',
  meson.project_source_root() / 'src' / 'config.rs',
  check: true,
)

cargo_build_script = find_program(
  meson.project_source_root() / 'build-aux' / 'cargo-build.sh',
)
cargo_profile = get_option('buildtype').startswith('debug') ? 'debug' : 'release'

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  command: [
    cargo_build_script,
    meson.project_source_root() / 'Cargo.toml',
    meson.project_build_root() / 'cargo-target',
    cargo_profile,
    '@OUTPUT@',
  ],
)

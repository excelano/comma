blueprint_compiler = find_program('blueprint-compiler')

subdir('gtk')

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
# Meson names three kinds of build and Cargo names two, so the middle one is
# spelled out in Cargo.toml rather than rounded to either side. Rounding it down
# is what this did before, and it cost a second to draw a wide file in the build
# most people run from source.
cargo_profile = {
  'debug': 'debug',
  'debugoptimized': 'debugoptimized',
}.get(get_option('buildtype'), 'release')

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
    cargo_profile,
    '@OUTPUT@',
  ],
)

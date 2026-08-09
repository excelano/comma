project(
  'comma',
  'rust',
  version: '0.1.0',
  license: 'MIT',
  meson_version: '>= 1.0.0',
)

i18n = import('i18n')
gnome = import('gnome')

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

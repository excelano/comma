# The blueprint sits in its own directory so that the compiled .ui lands beside
# it under the build tree, at src/gtk/window.ui, which is the path
# comma.gresource.xml looks for.
#
# It is compiled one file at a time, and named. blueprint-compiler also has a
# batch mode that takes a directory to write into, and this used that until the
# directory turned out to be the problem: a custom_target whose output is a
# directory is a target ninja considers up to date for as long as the directory
# exists, which is always. Editing the blueprint then rebuilt nothing, and an
# install shipped whatever .ui had last been compiled -- a resource bundle from
# a fortnight earlier, in the one case that was caught. Naming the file gives
# ninja something whose timestamp means what it says.
blueprints = custom_target(
  'blueprints',
  input: files('window.blp'),
  output: 'window.ui',
  command: [
    blueprint_compiler,
    'compile',
    '--output',
    '@OUTPUT@',
    '@INPUT@',
  ],
)

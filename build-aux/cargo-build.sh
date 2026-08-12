#!/usr/bin/env bash
#
# Bridge between Meson and Cargo.
#
# Meson owns configuration, resources, and installation; Cargo owns compilation.
# Meson expects a build step to produce a file at a path it chose, so this runs
# the Cargo build and copies the binary there.
#
# The target directory is Cargo's own rather than one of Meson's choosing, so
# this build shares its compiled dependencies with `cargo test` and with every
# other Rust project on the machine. Overriding it would quietly duplicate
# several hundred megabytes of compiled GTK bindings.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

set -euo pipefail

manifest=$1
profile=$2  # debug | debugoptimized | release
output=$3

target_dir=$(
    cargo metadata --format-version 1 --no-deps --manifest-path "$manifest" |
        sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p'
)

if [ -z "$target_dir" ]; then
    echo "cargo-build.sh: could not determine Cargo's target directory" >&2
    exit 1
fi

# Cargo's two built-in profiles are asked for by flag and any other by name.
# All three put the binary in a directory named after the profile.
args=(build --manifest-path "$manifest")
case "$profile" in
    debug) ;;
    release) args+=(--release) ;;
    *) args+=(--profile "$profile") ;;
esac

cargo "${args[@]}"
cp "$target_dir/$profile/comma" "$output"

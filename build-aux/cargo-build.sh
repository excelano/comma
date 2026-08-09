#!/usr/bin/env bash
#
# Bridge between Meson and Cargo.
#
# Meson owns configuration, resources, and installation; Cargo owns compilation.
# Meson expects a build step to produce a file at a path it chose, so this runs
# the Cargo build and copies the binary there.
#
# Author: David M. Anderson
# Built with AI assistance (Claude, Anthropic)

set -euo pipefail

manifest=$1
target_dir=$2
profile=$3  # debug | release
output=$4

args=(build --manifest-path "$manifest" --target-dir "$target_dir")
if [ "$profile" = release ]; then
    args+=(--release)
fi

cargo "${args[@]}"
cp "$target_dir/$profile/comma" "$output"

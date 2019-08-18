#!/bin/bash

# Requires the CHANNEL env var to be set to `debug` or `release.`

set -e
cd $(dirname "$0")

pushd ../ >/dev/null
source ./config.sh
popd >/dev/null

# Cleanup for previous run
#     v Clean target dir except for build scripts and incremental cache
rm -r target/*/{debug,release}/{build,deps,examples,libsysroot*,native} || true
rm -r sysroot_src/src/{libcore,libtest}/target/$TARGET_TRIPLE/$sysroot_channel/ || true
rm Cargo.lock test_target/Cargo.lock 2>/dev/null || true
rm -r sysroot 2>/dev/null || true

# Build libs
export RUSTFLAGS="$RUSTFLAGS -Z force-unstable-if-unmarked"
if [[ "$1" == "--release" ]]; then
    sysroot_channel='release'
    RUSTFLAGS="$RUSTFLAGS -Zmir-opt-level=3" cargo build --target $TARGET_TRIPLE --release
else
    sysroot_channel='debug'
    cargo build --target $TARGET_TRIPLE
fi

# Copy files to sysroot
mkdir -p sysroot/lib/rustlib/$TARGET_TRIPLE/lib/
cp -r target/$TARGET_TRIPLE/$sysroot_channel/deps/* sysroot/lib/rustlib/$TARGET_TRIPLE/lib/

if [[ "$1" == "--release" ]]; then
    channel='release'
    RUSTFLAGS="$RUSTFLAGS -Zmir-opt-level=3" cargo build --target $TARGET_TRIPLE --release --manifest-path ./sysroot_src/src/libtest/Cargo.toml
else
    channel='debug'
    cargo build --target $TARGET_TRIPLE --manifest-path ./sysroot_src/src/libtest/Cargo.toml
fi

# Copy files to sysroot
cp -r sysroot_src/src/libtest/target/$TARGET_TRIPLE/$sysroot_channel/deps/*  sysroot/lib/rustlib/$TARGET_TRIPLE/lib/

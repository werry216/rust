#!/bin/bash
# We use the WIX toolset to create combined installers for Windows, and these
# binaries are downloaded from https://github.com/wixtoolset/wix3 originally

set -euo pipefail
IFS=$'\n\t'

source "$(cd "$(dirname "$0")" && pwd)/../shared.sh"

if isWindows; then
    curl -O https://rust-lang-ci-mirrors.s3-us-west-1.amazonaws.com/rustc/wix311-binaries.zip
    mkdir -p wix/bin
    cd wix/bin
    7z x ../../wix311-binaries.zip

    ciCommandSetEnv WIX "$(pwd)/wix"
fi

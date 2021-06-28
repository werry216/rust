#!/bin/bash
# Checks that no error code explanation is removed.

set -euo pipefail

echo "Check if an error code explanation was removed..."

if (git diff "$BASE_COMMIT" --name-status | grep '^D' \
        | grep --quiet "compiler/rustc_error_codes/src/error_codes/"); then
    echo "Error code explanations should never be removed!"
    echo "Take a look at E0001 to see how to handle it."
    exit 1
fi

echo "No error code explanation was removed!"

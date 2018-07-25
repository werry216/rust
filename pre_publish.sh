#!/bin/bash

set -e

./util/update_lints.py

git status --short | sort | grep -v README.md | grep -v helper.txt | sort > helper.txt

# abort if the files differ
diff "publish.files" "helper.txt"

rm helper.txt

# add all changed files
git add .
git commit -m "Bump the version"

set +e

echo "Running \`cargo fmt\`.."

cd clippy_lints && cargo fmt -- --write-mode=overwrite && cd ..
cargo fmt -- --write-mode=overwrite

echo "Running tests to make sure \`cargo fmt\` did not break anything.."

cargo test

echo "If the tests passed, review and commit the formatting changes and remember to add a git tag."

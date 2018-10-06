# Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
# file at the top-level directory of this distribution and at
# http://rust-lang.org/COPYRIGHT.
#
# Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
# http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
# <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
# option. This file may not be copied, modified, or distributed
# except according to those terms.


set -ex

echo "Running clippy base tests"

PATH=$PATH:./node_modules/.bin
remark -f *.md > /dev/null
# build clippy in debug mode and run tests
cargo build --features debugging
cargo test --features debugging
cd clippy_lints && cargo test && cd ..
cd rustc_tools_util && cargo test && cd ..
# check that the lint lists are up-to-date
./util/update_lints.py -c
mkdir -p ~/rust/cargo/bin
cp target/debug/cargo-clippy ~/rust/cargo/bin/cargo-clippy
cp target/debug/clippy-driver ~/rust/cargo/bin/clippy-driver
rm ~/.cargo/bin/cargo-clippy
# run clippy on its own codebase...
PATH=$PATH:~/rust/cargo/bin cargo clippy --all-targets --all-features -- -D clippy::all -D clippy::internal
# ... and some test directories
cd clippy_workspace_tests && PATH=$PATH:~/rust/cargo/bin cargo clippy -- -D clippy::all && cd ..
cd clippy_workspace_tests/src && PATH=$PATH:~/rust/cargo/bin cargo clippy -- -D clippy::all && cd ../..
cd clippy_workspace_tests/subcrate && PATH=$PATH:~/rust/cargo/bin cargo clippy -- -D clippy::all && cd ../..
cd clippy_workspace_tests/subcrate/src && PATH=$PATH:~/rust/cargo/bin cargo clippy -- -D clippy::all && cd ../../..
cd clippy_dev && PATH=$PATH:~/rust/cargo/bin cargo clippy -- -D clippy::all && cd ..
cd rustc_tools_util/ && PATH=$PATH:~/rust/cargo/bin cargo clippy -- -D clippy::all && cd ..

# test --manifest-path
PATH=$PATH:~/rust/cargo/bin cargo clippy --manifest-path=clippy_workspace_tests/Cargo.toml -- -D clippy::all
cd clippy_workspace_tests/subcrate && PATH=$PATH:~/rust/cargo/bin cargo clippy --manifest-path=../Cargo.toml -- -D clippy::all && cd ../..
set +x

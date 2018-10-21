#!/bin/bash

set -e

unamestr=`uname`
if [[ "$unamestr" == 'Linux' ]]; then
   dylib_ext='so'
elif [[ "$unamestr" == 'Darwin' ]]; then
   dylib_ext='dylib'
else
   echo "Unsupported os"
   exit 1
fi

build_lib() {
    SHOULD_CODEGEN=1 $RUSTC $2 --crate-name $1 --crate-type lib
}

run_bin() {
    SHOULD_RUN=1 $RUSTC $@ --crate-type bin
}

build_example_bin() {
    $RUSTC $2 --crate-name $1 --crate-type bin

    pushd target/out
    gcc $1 libmini_core.rlib -o $1_bin
    sh -c ./$1_bin || true
    popd
}

if [[ "$1" == "--release" ]]; then
    channel='release'
    cargo build --release
else
    channel='debug'
    cargo build
fi

export RUSTFLAGS='-Zalways-encode-mir -Cpanic=abort -Zcodegen-backend='$(pwd)'/target/'$channel'/librustc_codegen_cranelift.'$dylib_ext
RUSTC="rustc $RUSTFLAGS -L crate=target/out --out-dir target/out"

rm -r target/out || true
mkdir -p target/out/clif

echo "[BUILD] mini_core"
build_lib mini_core example/mini_core.rs

$RUSTC example/example.rs --crate-type lib

echo "[JIT] mini_core_hello_world"
run_bin example/mini_core_hello_world.rs --cfg jit

echo "[AOT] mini_core_hello_world"
build_example_bin mini_core_hello_world example/mini_core_hello_world.rs

echo "[BUILD] core"
time $RUSTC target/libcore/src/libcore/lib.rs --crate-type lib --crate-name core -Cincremental=target/incremental_core

pushd xargo
rm -r ~/.xargo/HOST || true
export XARGO_RUST_SRC=$(pwd)'/../target/libcore/src'
time SHOULD_CODEGEN=1 xargo build --color always
popd

cat target/out/log.txt | sort | uniq -c

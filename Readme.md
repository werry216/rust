# WIP Cranelift codegen backend for rust

> ⚠⚠⚠ Certain kinds of FFI don't work yet. ⚠⚠⚠

## Building

```bash
$ git clone https://github.com/bjorn3/rustc_codegen_cranelift.git
$ cd rustc_codegen_cranelift
$ ./prepare.sh # download and patch sysroot src and install hyperfine for benchmarking
$ ./test.sh --release
```

## Usage

`$cg_clif_dir` is the directory you cloned this repo into in the following instructions.

### Cargo

```bash
$ CHANNEL="release" $cg_clif_dir/cargo.sh run
```

If you compiled cg_clif in debug mode (aka you didn't pass `--release` to `./test.sh`) you should use `CHANNEL="debug"` instead or omit `CHANNEL="release"` completely.

### Rustc

> You should prefer using the Cargo method.

```bash
$ rustc +$(cat $cg_clif_dir/rust-toolchain) -Cpanic=abort -Zcodegen-backend=$cg_clif_dir/target/release/librustc_codegen_cranelift.so --sysroot $cg_clif_dir/build_sysroot/sysroot my_crate.rs
```

### Shell

These are a few functions that allow you to easily run rust code from the shell using cg_clif as jit.

```bash
function jit_naked() {
    echo "$@" | CG_CLIF_JIT=1 rustc -Zcodegen-backend=$cg_clif_dir/target/release/librustc_codegen_cranelift.so --sysroot $cg_clif_dir/build_sysroot/sysroot - -Cprefer-dynamic
}

function jit() {
    jit_naked "fn main() { $@ }"
}

function jit_calc() {
    jit 'println!("0x{:x}", ' $@ ');';
}
```

## Env vars

<dl>
    <dt>CG_CLIF_JIT</dt>
    <dd>Enable JIT mode to immediately run a program instead of writing an executable file.</dd>
    <dt>CG_CLIF_JIT_ARGS</dt>
    <dd>When JIT mode is enable pass these arguments to the program.</dd>
    <dt>CG_CLIF_INCR_CACHE_DISABLED</dt>
    <dd>Don't cache object files in the incremental cache. Useful during development of cg_clif
    to make it possible to use incremental mode for all analyses performed by rustc without caching
    object files when their content should have been changed by a change to cg_clif.</dd>
    <dt>CG_CLIF_DISPLAY_CG_TIME</dt>
    <dd>Display the time it took to perform codegen for a crate</dd>
</dl>

## Not yet supported

* Good non-rust abi support ([several problems](https://github.com/bjorn3/rustc_codegen_cranelift/issues/10))
    * Proc macros
* Inline assembly ([no cranelift support](https://github.com/bytecodealliance/wasmtime/issues/1041), not coming soon)
* SIMD ([tracked here](https://github.com/bjorn3/rustc_codegen_cranelift/issues/171), some basic things work)

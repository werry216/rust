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

TARGET_TRIPLE=$(rustc -vV | grep host | cut -d: -f2 | tr -d " ")

export RUSTFLAGS='-Cpanic=abort -Cdebuginfo=2 -Zpanic-abort-tests -Zcodegen-backend='$(pwd)'/target/'$CHANNEL'/librustc_codegen_cranelift.'$dylib_ext' --sysroot '$(pwd)'/build_sysroot/sysroot'

# FIXME remove once the atomic shim is gone
if [[ `uname` == 'Darwin' ]]; then
   export RUSTFLAGS="$RUSTFLAGS -Clink-arg=-undefined -Clink-arg=dynamic_lookup"
fi

RUSTC="rustc $RUSTFLAGS -L crate=target/out --out-dir target/out"
export RUSTC_LOG=warn # display metadata load errors

export LD_LIBRARY_PATH="$(pwd)/target/out:$(pwd)/build_sysroot/sysroot/lib/rustlib/$TARGET_TRIPLE/lib"
export DYLD_LIBRARY_PATH=$LD_LIBRARY_PATH

export CG_CLIF_DISPLAY_CG_TIME=1
export CG_CLIF_INCR_CACHE_DISABLED=1

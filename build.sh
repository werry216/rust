cargo build || exit 1

cd examples/

unamestr=`uname`
if [[ "$unamestr" == 'Linux' ]]; then
   dylib_ext='so'
elif [[ "$unamestr" == 'Darwin' ]]; then
   dylib_ext='dylib'
else
   echo "Unsupported os"
   exit 1
fi

RUSTC="rustc -Zcodegen-backend=$(pwd)/../target/debug/librustc_codegen_cranelift.$dylib_ext -L crate=. --crate-type lib"

$RUSTC mini_core.rs --crate-name mini_core &&
$RUSTC example.rs &&
$RUSTC mini_core_hello_world.rs &&
$RUSTC ../target/libcore/src/libcore/lib.rs &&

rm *.rlib

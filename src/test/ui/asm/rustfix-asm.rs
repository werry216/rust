// run-rustfix
// only-x86_64

#![feature(asm, llvm_asm)]

fn main() {
    unsafe {
        let x = 1;
        let y: i32;
        asm!("" :: "r" (x));
        //~^ ERROR the legacy LLVM-style asm! syntax is no longer supported
        asm!("" : "=r" (y));
        //~^ ERROR the legacy LLVM-style asm! syntax is no longer supported
        let _ = y;
    }
}

// run-rustfix
// edition:2018
// check-pass

trait TryIntoU32 {
    fn try_into(self) -> Result<u32, ()>;
}

impl TryIntoU32 for u8 {
    fn try_into(self) -> Result<u32, ()> {
        Ok(self as u32)
    }
}

trait TryFromU8: Sized {
    fn try_from(x: u8) -> Result<Self, ()>;
}

impl TryFromU8 for u32 {
    fn try_from(x: u8) -> Result<Self, ()> {
        Ok(x as u32)
    }
}

trait FromByteIterator {
    fn from_iter<T>(iter: T) -> Self
        where T: Iterator<Item = u8>;
}

impl FromByteIterator for Vec<u8> {
    fn from_iter<T>(iter: T) -> Self
        where T: Iterator<Item = u8>
    {
        iter.collect()
    }
}

fn main() {
    // test dot-call that will break in 2021 edition
    let _: u32 = 3u8.try_into().unwrap();
    //~^ WARNING trait method `try_into` will become ambiguous in Rust 2021

    // test associated function call that will break in 2021 edition
    let _ = u32::try_from(3u8).unwrap();
    //~^ WARNING trait-associated function `try_from` will become ambiguous in Rust 2021

    // test reverse turbofish too
    let _ = <Vec<u8>>::from_iter(vec![1u8, 2, 3, 4, 5, 6].into_iter());
    //~^ WARNING trait-associated function `from_iter` will become ambiguous in Rust 2021

    // negative testing lint (this line should *not* emit a warning)
    let _: u32 = TryFromU8::try_from(3u8).unwrap();

    // test type omission
    let _: u32 = <_>::try_from(3u8).unwrap();
    //~^ WARNING trait-associated function `try_from` will become ambiguous in Rust 2021
}

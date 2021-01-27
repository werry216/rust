// run-pass
#![feature(const_evaluatable_checked, const_generics)]
#![allow(incomplete_features)]

struct Generic<const K: u64>;

struct ConstU64<const K: u64>;

impl<const K: u64> Generic<K>
where
    ConstU64<{ K - 1 }>: ,
{
    fn foo(self) -> u64 {
        K
    }
}

impl<const K: u64> Generic<K>
where
    ConstU64<{ K - 1 }>: ,
    ConstU64<{ K + 1 }>: ,
    ConstU64<{ K + 1 - 1 }>: ,
{
    fn bar(self) -> u64 {
        let x: Generic<{ K + 1 }> = Generic;
        x.foo()
    }
}

fn main() {
    assert_eq!((Generic::<10>).bar(), 11);
}

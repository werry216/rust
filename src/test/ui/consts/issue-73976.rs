// This test is from #73976. We previously did not check if a type is monomorphized
// before calculating its type id, which leads to the bizzare behaviour below that
// TypeId of a generic type does not match itself.
//
// This test case should either run-pass or be rejected at compile time.
// Currently we just disallow this usage and require pattern is monomorphic.

#![feature(const_type_id)]

use std::any::TypeId;

pub struct GetTypeId<T>(T);

impl<T: 'static> GetTypeId<T> {
    pub const VALUE: TypeId = TypeId::of::<T>();
}

const fn check_type_id<T: 'static>() -> bool {
    matches!(GetTypeId::<T>::VALUE, GetTypeId::<T>::VALUE)
    //~^ ERROR could not evaluate constant pattern
    //~| ERROR could not evaluate constant pattern
}

fn main() {
    assert!(check_type_id::<usize>());
}

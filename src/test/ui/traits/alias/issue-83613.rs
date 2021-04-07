#![feature(min_type_alias_impl_trait)]
trait OpaqueTrait {}
impl<T> OpaqueTrait for T {}
type OpaqueType = impl OpaqueTrait;
fn mk_opaque() -> OpaqueType {
    || 0
}
trait AnotherTrait {}
impl<T: Send> AnotherTrait for T {}
impl AnotherTrait for OpaqueType {}
//~^ ERROR conflicting implementations of trait `AnotherTrait` for type `impl OpaqueTrait`
//~| ERROR cannot implement trait on type alias impl trait
fn main() {}

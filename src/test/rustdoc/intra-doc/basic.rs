// @has basic/index.html
// @has - '//a/@href' 'struct.ThisType.html'
// @has - '//a/@href' 'struct.ThisType.html#method.this_method'
// @has - '//a/@href' 'enum.ThisEnum.html'
// @has - '//a/@href' 'enum.ThisEnum.html#variant.ThisVariant'
// @has - '//a/@href' 'trait.ThisTrait.html'
// @has - '//a/@href' 'trait.ThisTrait.html#tymethod.this_associated_method'
// @has - '//a/@href' 'trait.ThisTrait.html#associatedtype.ThisAssociatedType'
// @has - '//a/@href' 'trait.ThisTrait.html#associatedconstant.THIS_ASSOCIATED_CONST'
// @has - '//a/@href' 'trait.ThisTrait.html'
// @has - '//a/@href' 'type.ThisAlias.html'
// @has - '//a/@href' 'union.ThisUnion.html'
// @has - '//a/@href' 'fn.this_function.html'
// @has - '//a/@href' 'constant.THIS_CONST.html'
// @has - '//a/@href' 'static.THIS_STATIC.html'
// @has - '//a/@href' 'macro.this_macro.html'
// @has - '//a/@href' 'trait.SoAmbiguous.html'
// @has - '//a/@href' 'fn.SoAmbiguous.html'
//! In this crate we would like to link to:
//!
//! * [`ThisType`](ThisType)
//! * [`ThisType::this_method`](ThisType::this_method)
//! * [`ThisEnum`](ThisEnum)
//! * [`ThisEnum::ThisVariant`](ThisEnum::ThisVariant)
//! * [`ThisEnum::ThisVariantCtor`](ThisEnum::ThisVariantCtor)
//! * [`ThisTrait`](ThisTrait)
//! * [`ThisTrait::this_associated_method`](ThisTrait::this_associated_method)
//! * [`ThisTrait::ThisAssociatedType`](ThisTrait::ThisAssociatedType)
//! * [`ThisTrait::THIS_ASSOCIATED_CONST`](ThisTrait::THIS_ASSOCIATED_CONST)
//! * [`ThisAlias`](ThisAlias)
//! * [`ThisUnion`](ThisUnion)
//! * [`this_function`](this_function())
//! * [`THIS_CONST`](const@THIS_CONST)
//! * [`THIS_STATIC`](static@THIS_STATIC)
//! * [`this_macro`](this_macro!)
//!
//! In addition, there's some specifics we want to look at. There's [a trait called
//! SoAmbiguous][ambig-trait], but there's also [a function called SoAmbiguous][ambig-fn] too!
//! Whatever shall we do?
//!
//! [ambig-trait]: trait@SoAmbiguous
//! [ambig-fn]: SoAmbiguous()

#[macro_export]
macro_rules! this_macro {
    () => {};
}

// @has basic/struct.ThisType.html '//a/@href' 'macro.this_macro.html'
/// another link to [`this_macro!()`]
pub struct ThisType;

impl ThisType {
    pub fn this_method() {}
}
pub enum ThisEnum { ThisVariant, ThisVariantCtor(u32), }
pub trait ThisTrait {
    type ThisAssociatedType;
    const THIS_ASSOCIATED_CONST: u8;
    fn this_associated_method();
}
pub type ThisAlias = Result<(), ()>;
pub union ThisUnion { this_field: usize, }

pub fn this_function() {}
pub const THIS_CONST: usize = 5usize;
pub static THIS_STATIC: usize = 5usize;

pub trait SoAmbiguous {}

#[allow(nonstandard_style)]
pub fn SoAmbiguous() {}


// @has basic/struct.SomeOtherType.html '//a/@href' 'struct.ThisType.html'
// @has - '//a/@href' 'struct.ThisType.html#method.this_method'
// @has - '//a/@href' 'enum.ThisEnum.html'
// @has - '//a/@href' 'enum.ThisEnum.html#variant.ThisVariant'
/// Shortcut links for:
/// * [`ThisType`]
/// * [`ThisType::this_method`]
/// * [ThisEnum]
/// * [ThisEnum::ThisVariant]
pub struct SomeOtherType;

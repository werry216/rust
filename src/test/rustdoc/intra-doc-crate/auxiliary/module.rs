#![crate_name = "module_inner"]
#![deny(intra_doc_resolution_failures)]
/// [SomeType] links to [bar]
pub struct SomeType;
pub trait SomeTrait {}
/// [bar] links to [SomeTrait] and also [SomeType]
pub mod bar {}

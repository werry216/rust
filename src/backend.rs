use std::collections::HashMap;

use rustc::session::Session;

use cranelift_module::{FuncId, Module};

use faerie::*;
use object::{SectionKind, RelocationKind, RelocationEncoding};
use object::write::*;
use cranelift_faerie::{FaerieBackend, FaerieBuilder, FaerieProduct, FaerieTrapCollection};
use cranelift_object::*;

use gimli::SectionId;

use crate::debuginfo::{DebugReloc, DebugRelocName};

pub trait WriteMetadata {
    fn add_rustc_section(&mut self, symbol_name: String, data: Vec<u8>, is_like_osx: bool);
}

impl WriteMetadata for faerie::Artifact {
    fn add_rustc_section(&mut self, symbol_name: String, data: Vec<u8>, is_like_osx: bool) {
        self
            .declare(".rustc", faerie::Decl::section(faerie::SectionKind::Data))
            .unwrap();
        self
            .define_with_symbols(".rustc", data, {
                let mut map = std::collections::BTreeMap::new();
                // FIXME implement faerie elf backend section custom symbols
                // For MachO this is necessary to prevent the linker from throwing away the .rustc section,
                // but for ELF it isn't.
                if is_like_osx {
                    map.insert(
                        symbol_name,
                        0,
                    );
                }
                map
            })
            .unwrap();
    }
}

impl WriteMetadata for object::write::Object {
    fn add_rustc_section(&mut self, symbol_name: String, data: Vec<u8>, is_like_osx: bool) {
        let segment = self.segment_name(object::write::StandardSegment::Data).to_vec();
        let section_id = self.add_section(segment, b".rustc".to_vec(), object::SectionKind::Data);
        let offset = self.append_section_data(section_id, &data, 1);
        // FIXME implement faerie elf backend section custom symbols
        // For MachO this is necessary to prevent the linker from throwing away the .rustc section,
        // but for ELF it isn't.
        if is_like_osx {
            self.add_symbol(object::write::Symbol {
                name: symbol_name.into_bytes(),
                value: offset,
                size: data.len() as u64,
                kind: object::SymbolKind::Data,
                scope: object::SymbolScope::Compilation,
                weak: false,
                section: Some(section_id),
            });
        }
    }
}

pub trait WriteDebugInfo {
    type SectionId;

    fn add_debug_section(&mut self, name: SectionId, data: Vec<u8>) -> Self::SectionId;
    fn add_debug_reloc(
        &mut self,
        section_map: &HashMap<SectionId, Self::SectionId>,
        symbol_map: &indexmap::IndexSet<(String, FuncId)>,
        from: &Self::SectionId,
        reloc: &DebugReloc,
    );
}

impl WriteDebugInfo for FaerieProduct {
    type SectionId = SectionId;

    fn add_debug_section(&mut self, id: SectionId, data: Vec<u8>) -> SectionId {
        self.artifact.declare_with(id.name(), Decl::section(faerie::SectionKind::Debug), data).unwrap();
        id
    }

    fn add_debug_reloc(
        &mut self,
        _section_map: &HashMap<SectionId, Self::SectionId>,
        symbol_map: &indexmap::IndexSet<(String, FuncId)>,
        from: &Self::SectionId,
        reloc: &DebugReloc,
    ) {
        self
            .artifact
            .link_with(
                faerie::Link {
                    from: from.name(),
                    to: match reloc.name {
                        DebugRelocName::Section(id) => id.name(),
                        DebugRelocName::Symbol(index) => &symbol_map.get_index(index).unwrap().0,
                    },
                    at: u64::from(reloc.offset),
                },
                faerie::Reloc::Debug {
                    size: reloc.size,
                    addend: reloc.addend as i32,
                },
            )
            .expect("faerie relocation error");
    }
}

impl WriteDebugInfo for ObjectProduct {
    type SectionId = (object::write::SectionId, object::write::SymbolId);

    fn add_debug_section(
        &mut self,
        id: SectionId,
        data: Vec<u8>,
    ) -> (object::write::SectionId, object::write::SymbolId) {
        let segment = self.object.segment_name(StandardSegment::Debug).to_vec();
        let name = id.name().as_bytes().to_vec();
        let section_id = self.object.add_section(segment, name, SectionKind::Debug);
        self.object.section_mut(section_id).set_data(data, 1);
        let symbol_id = self.object.section_symbol(section_id);
        (section_id, symbol_id)
    }

    fn add_debug_reloc(
        &mut self,
        section_map: &HashMap<SectionId, Self::SectionId>,
        symbol_map: &indexmap::IndexSet<(String, FuncId)>,
        from: &Self::SectionId,
        reloc: &DebugReloc,
    ) {
        let symbol = match reloc.name {
            DebugRelocName::Section(id) => section_map.get(&id).unwrap().1,
            DebugRelocName::Symbol(id) => {
                let (_func_name, func_id) = symbol_map.get_index(id).unwrap();
                self.function_symbol(*func_id)
            }
        };
        self.object.add_relocation(from.0, Relocation {
            offset: u64::from(reloc.offset),
            symbol,
            kind: RelocationKind::Absolute,
            encoding: RelocationEncoding::Generic,
            size: reloc.size * 8,
            addend: reloc.addend,
        }).unwrap();
    }
}

pub trait Emit {
    fn emit(self) -> Vec<u8>;
}

impl Emit for FaerieProduct {
    fn emit(self) -> Vec<u8> {
        self.artifact.emit().unwrap()
    }
}

impl Emit for ObjectProduct {
    fn emit(self) -> Vec<u8> {
        self.object.write().unwrap()
    }
}

#[cfg(not(feature = "backend_object"))]
pub fn with_object(sess: &Session, name: &str, f: impl FnOnce(&mut Artifact)) -> Vec<u8> {
    let mut metadata_artifact = faerie::Artifact::new(
        crate::build_isa(sess, true).triple().clone(),
        name.to_string(),
    );
    f(&mut metadata_artifact);
    metadata_artifact.emit().unwrap()
}

#[cfg(feature = "backend_object")]
pub fn with_object(sess: &Session, name: &str, f: impl FnOnce(&mut Object)) -> Vec<u8> {
    let triple = crate::build_isa(sess, true).triple().clone();
    let mut metadata_object =
        object::write::Object::new(triple.binary_format, triple.architecture);
    metadata_object.add_file_symbol(name.as_bytes().to_vec());
    f(&mut metadata_object);
    metadata_object.write().unwrap()
}

pub type Backend = impl cranelift_module::Backend<Product: Emit + WriteDebugInfo>;

#[cfg(not(feature = "backend_object"))]
pub fn make_module(sess: &Session, name: String) -> Module<Backend> {
    let module: Module<FaerieBackend> = Module::new(
        FaerieBuilder::new(
            crate::build_isa(sess, true),
            name + ".o",
            FaerieTrapCollection::Disabled,
            cranelift_module::default_libcall_names(),
        )
        .unwrap(),
    );
    module
}

#[cfg(feature = "backend_object")]
pub fn make_module(sess: &Session, name: String) -> Module<Backend> {
    let module: Module<ObjectBackend> = Module::new(
        ObjectBuilder::new(
            crate::build_isa(sess, true),
            name + ".o",
            ObjectTrapCollection::Disabled,
            cranelift_module::default_libcall_names(),
        )
        .unwrap(),
    );
    module
}

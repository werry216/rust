use std::convert::{TryFrom, TryInto};

use rustc_data_structures::fx::FxHashMap;
use rustc_session::Session;

use cranelift_module::{FuncId, Module};

use object::{SectionKind, SymbolFlags, RelocationKind, RelocationEncoding};
use object::write::*;

use cranelift_object::*;

use gimli::SectionId;

use crate::debuginfo::{DebugReloc, DebugRelocName};

pub(crate) trait WriteMetadata {
    fn add_rustc_section(&mut self, symbol_name: String, data: Vec<u8>, is_like_osx: bool);
}

impl WriteMetadata for object::write::Object {
    fn add_rustc_section(&mut self, symbol_name: String, data: Vec<u8>, _is_like_osx: bool) {
        let segment = self.segment_name(object::write::StandardSegment::Data).to_vec();
        let section_id = self.add_section(segment, b".rustc".to_vec(), object::SectionKind::Data);
        let offset = self.append_section_data(section_id, &data, 1);
        // For MachO and probably PE this is necessary to prevent the linker from throwing away the
        // .rustc section. For ELF this isn't necessary, but it also doesn't harm.
        self.add_symbol(object::write::Symbol {
            name: symbol_name.into_bytes(),
            value: offset,
            size: data.len() as u64,
            kind: object::SymbolKind::Data,
            scope: object::SymbolScope::Dynamic,
            weak: false,
            section: SymbolSection::Section(section_id),
            flags: SymbolFlags::None,
        });
    }
}

pub(crate) trait WriteDebugInfo {
    type SectionId: Copy;

    fn add_debug_section(&mut self, name: SectionId, data: Vec<u8>) -> Self::SectionId;
    fn add_debug_reloc(
        &mut self,
        section_map: &FxHashMap<SectionId, Self::SectionId>,
        from: &Self::SectionId,
        reloc: &DebugReloc,
    );
}

impl WriteDebugInfo for ObjectProduct {
    type SectionId = (object::write::SectionId, object::write::SymbolId);

    fn add_debug_section(
        &mut self,
        id: SectionId,
        data: Vec<u8>,
    ) -> (object::write::SectionId, object::write::SymbolId) {
        let name = if self.object.format() == target_lexicon::BinaryFormat::Macho {
            id.name().replace('.', "__") // machO expects __debug_info instead of .debug_info
        } else {
            id.name().to_string()
        }.into_bytes();

        let segment = self.object.segment_name(StandardSegment::Debug).to_vec();
        let section_id = self.object.add_section(segment, name, SectionKind::Debug);
        self.object.section_mut(section_id).set_data(data, 1);
        let symbol_id = self.object.section_symbol(section_id);
        (section_id, symbol_id)
    }

    fn add_debug_reloc(
        &mut self,
        section_map: &FxHashMap<SectionId, Self::SectionId>,
        from: &Self::SectionId,
        reloc: &DebugReloc,
    ) {
        let (symbol, symbol_offset) = match reloc.name {
            DebugRelocName::Section(id) => {
                (section_map.get(&id).unwrap().1, 0)
            }
            DebugRelocName::Symbol(id) => {
                let symbol_id = self.function_symbol(FuncId::from_u32(id.try_into().unwrap()));
                self.object.symbol_section_and_offset(symbol_id).expect("Debug reloc for undef sym???")
            }
        };
        self.object.add_relocation(from.0, Relocation {
            offset: u64::from(reloc.offset),
            symbol,
            kind: RelocationKind::Absolute,
            encoding: RelocationEncoding::Generic,
            size: reloc.size * 8,
            addend: i64::try_from(symbol_offset).unwrap() + reloc.addend,
        }).unwrap();
    }
}

pub(crate) trait Emit {
    fn emit(self) -> Vec<u8>;
}

impl Emit for ObjectProduct {
    fn emit(self) -> Vec<u8> {
        self.object.write().unwrap()
    }
}

pub(crate) fn with_object(sess: &Session, name: &str, f: impl FnOnce(&mut Object)) -> Vec<u8> {
    let triple = crate::build_isa(sess, true).triple().clone();
    let mut metadata_object =
        object::write::Object::new(triple.binary_format, triple.architecture);
    metadata_object.add_file_symbol(name.as_bytes().to_vec());
    f(&mut metadata_object);
    metadata_object.write().unwrap()
}

pub(crate) type Backend = impl cranelift_module::Backend<Product: Emit + WriteDebugInfo>;

pub(crate) fn make_module(sess: &Session, name: String) -> Module<Backend> {
    let module: Module<ObjectBackend> = Module::new(
        ObjectBuilder::new(
            crate::build_isa(sess, true),
            name + ".o",
            cranelift_module::default_libcall_names(),
        ),
    );
    module
}

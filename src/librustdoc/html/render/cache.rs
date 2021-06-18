use std::collections::BTreeMap;

use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_middle::ty::TyCtxt;
use rustc_span::symbol::Symbol;
use serde::ser::{Serialize, SerializeStruct, Serializer};

use crate::clean;
use crate::clean::types::{
    FnDecl, FnRetTy, GenericBound, Generics, GetDefId, Type, WherePredicate,
};
use crate::formats::cache::Cache;
use crate::formats::item_type::ItemType;
use crate::html::markdown::short_markdown_summary;
use crate::html::render::{Generic, IndexItem, IndexItemFunctionType, RenderType, TypeWithKind};

/// Indicates where an external crate can be found.
crate enum ExternalLocation {
    /// Remote URL root of the external crate
    Remote(String),
    /// This external crate can be found in the local doc/ folder
    Local,
    /// The external crate could not be found.
    Unknown,
}

/// Builds the search index from the collected metadata
crate fn build_index<'tcx>(krate: &clean::Crate, cache: &mut Cache, tcx: TyCtxt<'tcx>) -> String {
    let mut defid_to_pathid = FxHashMap::default();
    let mut crate_items = Vec::with_capacity(cache.search_index.len());
    let mut crate_paths = vec![];

    // Attach all orphan items to the type's definition if the type
    // has since been learned.
    for &(did, ref item) in &cache.orphan_impl_items {
        if let Some(&(ref fqp, _)) = cache.paths.get(&did) {
            cache.search_index.push(IndexItem {
                ty: item.type_(),
                name: item.name.unwrap().to_string(),
                path: fqp[..fqp.len() - 1].join("::"),
                desc: item.doc_value().map_or_else(String::new, |s| short_markdown_summary(&s)),
                parent: Some(did.into()),
                parent_idx: None,
                search_type: get_index_search_type(&item, cache, tcx),
                aliases: item.attrs.get_doc_aliases(),
            });
        }
    }

    let Cache { ref mut search_index, ref paths, .. } = *cache;

    // Aliases added through `#[doc(alias = "...")]`. Since a few items can have the same alias,
    // we need the alias element to have an array of items.
    let mut aliases: BTreeMap<String, Vec<usize>> = BTreeMap::new();

    // Sort search index items. This improves the compressibility of the search index.
    search_index.sort_unstable_by(|k1, k2| {
        // `sort_unstable_by_key` produces lifetime errors
        let k1 = (&k1.path, &k1.name, &k1.ty, &k1.parent);
        let k2 = (&k2.path, &k2.name, &k2.ty, &k2.parent);
        std::cmp::Ord::cmp(&k1, &k2)
    });

    // Set up alias indexes.
    for (i, item) in search_index.iter().enumerate() {
        for alias in &item.aliases[..] {
            aliases.entry(alias.to_lowercase()).or_insert(Vec::new()).push(i);
        }
    }

    // Reduce `DefId` in paths into smaller sequential numbers,
    // and prune the paths that do not appear in the index.
    let mut lastpath = String::new();
    let mut lastpathid = 0usize;

    for item in search_index {
        item.parent_idx = item.parent.and_then(|defid| {
            if defid_to_pathid.contains_key(&defid) {
                defid_to_pathid.get(&defid).copied()
            } else {
                let pathid = lastpathid;
                defid_to_pathid.insert(defid, pathid);
                lastpathid += 1;

                if let Some(&(ref fqp, short)) = paths.get(&defid) {
                    crate_paths.push((short, fqp.last().unwrap().clone()));
                    Some(pathid)
                } else {
                    None
                }
            }
        });

        // Omit the parent path if it is same to that of the prior item.
        if lastpath == item.path {
            item.path.clear();
        } else {
            lastpath = item.path.clone();
        }
        crate_items.push(&*item);
    }

    let crate_doc =
        krate.module.doc_value().map_or_else(String::new, |s| short_markdown_summary(&s));

    struct CrateData<'a> {
        doc: String,
        items: Vec<&'a IndexItem>,
        paths: Vec<(ItemType, String)>,
        // The String is alias name and the vec is the list of the elements with this alias.
        //
        // To be noted: the `usize` elements are indexes to `items`.
        aliases: &'a BTreeMap<String, Vec<usize>>,
    }

    impl<'a> Serialize for CrateData<'a> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            let has_aliases = !self.aliases.is_empty();
            let mut crate_data =
                serializer.serialize_struct("CrateData", if has_aliases { 9 } else { 8 })?;
            crate_data.serialize_field("doc", &self.doc)?;
            crate_data.serialize_field(
                "t",
                &self.items.iter().map(|item| &item.ty).collect::<Vec<_>>(),
            )?;
            crate_data.serialize_field(
                "n",
                &self.items.iter().map(|item| &item.name).collect::<Vec<_>>(),
            )?;
            crate_data.serialize_field(
                "q",
                &self.items.iter().map(|item| &item.path).collect::<Vec<_>>(),
            )?;
            crate_data.serialize_field(
                "d",
                &self.items.iter().map(|item| &item.desc).collect::<Vec<_>>(),
            )?;
            crate_data.serialize_field(
                "i",
                &self
                    .items
                    .iter()
                    .map(|item| {
                        assert_eq!(
                            item.parent.is_some(),
                            item.parent_idx.is_some(),
                            "`{}` is missing idx",
                            item.name
                        );
                        item.parent_idx.map(|x| x + 1).unwrap_or(0)
                    })
                    .collect::<Vec<_>>(),
            )?;
            crate_data.serialize_field(
                "f",
                &self.items.iter().map(|item| &item.search_type).collect::<Vec<_>>(),
            )?;
            crate_data.serialize_field("p", &self.paths)?;
            if has_aliases {
                crate_data.serialize_field("a", &self.aliases)?;
            }
            crate_data.end()
        }
    }

    // Collect the index into a string
    format!(
        r#""{}":{}"#,
        krate.name,
        serde_json::to_string(&CrateData {
            doc: crate_doc,
            items: crate_items,
            paths: crate_paths,
            aliases: &aliases,
        })
        .expect("failed serde conversion")
        // All these `replace` calls are because we have to go through JS string for JSON content.
        .replace(r"\", r"\\")
        .replace("'", r"\'")
        // We need to escape double quotes for the JSON.
        .replace("\\\"", "\\\\\"")
    )
}

crate fn get_index_search_type<'tcx>(
    item: &clean::Item,
    cache: &Cache,
    tcx: TyCtxt<'tcx>,
) -> Option<IndexItemFunctionType> {
    let (all_types, ret_types) = match *item.kind {
        clean::FunctionItem(ref f) => get_all_types(&f.generics, &f.decl, tcx),
        clean::MethodItem(ref m, _) => get_all_types(&m.generics, &m.decl, tcx),
        clean::TyMethodItem(ref m) => get_all_types(&m.generics, &m.decl, tcx),
        _ => return None,
    };

    let inputs = all_types
        .iter()
        .map(|(ty, kind)| TypeWithKind::from((get_index_type(&ty, &cache), *kind)))
        .filter(|a| a.ty.name.is_some())
        .collect();
    let output = ret_types
        .iter()
        .map(|(ty, kind)| TypeWithKind::from((get_index_type(&ty, &cache), *kind)))
        .filter(|a| a.ty.name.is_some())
        .collect::<Vec<_>>();
    let output = if output.is_empty() { None } else { Some(output) };

    Some(IndexItemFunctionType { inputs, output })
}

fn get_index_type(clean_type: &clean::Type, cache: &Cache) -> RenderType {
    RenderType {
        ty: clean_type.def_id_full(cache),
        idx: None,
        name: get_index_type_name(clean_type, true).map(|s| s.as_str().to_ascii_lowercase()),
        generics: get_generics(clean_type, cache),
    }
}

fn get_index_type_name(clean_type: &clean::Type, accept_generic: bool) -> Option<Symbol> {
    match *clean_type {
        clean::ResolvedPath { ref path, .. } => {
            let segments = &path.segments;
            let path_segment = segments.iter().last().unwrap_or_else(|| {
                panic!(
                "get_index_type_name(clean_type: {:?}, accept_generic: {:?}) had length zero path",
                clean_type, accept_generic
            )
            });
            Some(path_segment.name)
        }
        clean::DynTrait(ref bounds, _) => get_index_type_name(&bounds[0].trait_, accept_generic),
        clean::Generic(s) if accept_generic => Some(s),
        clean::Primitive(ref p) => Some(p.as_sym()),
        clean::BorrowedRef { ref type_, .. } => get_index_type_name(type_, accept_generic),
        clean::Generic(_)
        | clean::BareFunction(_)
        | clean::Tuple(_)
        | clean::Slice(_)
        | clean::Array(_, _)
        | clean::Never
        | clean::RawPointer(_, _)
        | clean::QPath { .. }
        | clean::Infer
        | clean::ImplTrait(_) => None,
    }
}

fn get_generics(clean_type: &clean::Type, cache: &Cache) -> Option<Vec<Generic>> {
    clean_type.generics().and_then(|types| {
        let r = types
            .iter()
            .filter_map(|t| {
                get_index_type_name(t, false).map(|name| Generic {
                    name: name.as_str().to_ascii_lowercase(),
                    defid: t.def_id_full(cache),
                    idx: None,
                })
            })
            .collect::<Vec<_>>();
        if r.is_empty() { None } else { Some(r) }
    })
}

/// The point of this function is to replace bounds with types.
///
/// i.e. `[T, U]` when you have the following bounds: `T: Display, U: Option<T>` will return
/// `[Display, Option]` (we just returns the list of the types, we don't care about the
/// wrapped types in here).
crate fn get_real_types<'tcx>(
    generics: &Generics,
    arg: &Type,
    tcx: TyCtxt<'tcx>,
    recurse: i32,
    res: &mut FxHashSet<(Type, ItemType)>,
) -> usize {
    fn insert(res: &mut FxHashSet<(Type, ItemType)>, tcx: TyCtxt<'_>, ty: Type) -> usize {
        if let Some(kind) = ty.def_id().map(|did| tcx.def_kind(did).into()) {
            res.insert((ty, kind));
            1
        } else if ty.is_primitive() {
            // This is a primitive, let's store it as such.
            res.insert((ty, ItemType::Primitive));
            1
        } else {
            0
        }
    }

    if recurse >= 10 {
        // FIXME: remove this whole recurse thing when the recursion bug is fixed
        return 0;
    }
    let mut nb_added = 0;

    if let &Type::Generic(arg_s) = arg {
        if let Some(where_pred) = generics.where_predicates.iter().find(|g| match g {
            WherePredicate::BoundPredicate { ty, .. } => ty.def_id() == arg.def_id(),
            _ => false,
        }) {
            let bounds = where_pred.get_bounds().unwrap_or_else(|| &[]);
            for bound in bounds.iter() {
                if let GenericBound::TraitBound(poly_trait, _) = bound {
                    for x in poly_trait.generic_params.iter() {
                        if !x.is_type() {
                            continue;
                        }
                        if let Some(ty) = x.get_type() {
                            let adds = get_real_types(generics, &ty, tcx, recurse + 1, res);
                            nb_added += adds;
                            if adds == 0 && !ty.is_full_generic() {
                                nb_added += insert(res, tcx, ty);
                            }
                        }
                    }
                }
            }
        }
        if let Some(bound) = generics.params.iter().find(|g| g.is_type() && g.name == arg_s) {
            for bound in bound.get_bounds().unwrap_or(&[]) {
                if let Some(ty) = bound.get_trait_type() {
                    let adds = get_real_types(generics, &ty, tcx, recurse + 1, res);
                    nb_added += adds;
                    if adds == 0 && !ty.is_full_generic() {
                        nb_added += insert(res, tcx, ty);
                    }
                }
            }
        }
    } else {
        nb_added += insert(res, tcx, arg.clone());
        if let Some(gens) = arg.generics() {
            for gen in gens.iter() {
                if gen.is_full_generic() {
                    nb_added += get_real_types(generics, gen, tcx, recurse + 1, res);
                } else {
                    nb_added += insert(res, tcx, (*gen).clone());
                }
            }
        }
    }
    nb_added
}

/// Return the full list of types when bounds have been resolved.
///
/// i.e. `fn foo<A: Display, B: Option<A>>(x: u32, y: B)` will return
/// `[u32, Display, Option]`.
crate fn get_all_types<'tcx>(
    generics: &Generics,
    decl: &FnDecl,
    tcx: TyCtxt<'tcx>,
) -> (Vec<(Type, ItemType)>, Vec<(Type, ItemType)>) {
    let mut all_types = FxHashSet::default();
    for arg in decl.inputs.values.iter() {
        if arg.type_.is_self_type() {
            continue;
        }
        let mut args = FxHashSet::default();
        get_real_types(generics, &arg.type_, tcx, 0, &mut args);
        if !args.is_empty() {
            all_types.extend(args);
        } else {
            if let Some(kind) = arg.type_.def_id().map(|did| tcx.def_kind(did).into()) {
                all_types.insert((arg.type_.clone(), kind));
            }
        }
    }

    let ret_types = match decl.output {
        FnRetTy::Return(ref return_type) => {
            let mut ret = FxHashSet::default();
            get_real_types(generics, &return_type, tcx, 0, &mut ret);
            if ret.is_empty() {
                if let Some(kind) = return_type.def_id().map(|did| tcx.def_kind(did).into()) {
                    ret.insert((return_type.clone(), kind));
                }
            }
            ret.into_iter().collect()
        }
        _ => Vec::new(),
    };
    (all_types.into_iter().collect(), ret_types)
}

use crate::utils::{in_macro, snippet, span_lint_and_sugg};
use hir::def::{DefKind, Res};
use if_chain::if_chain;
use rustc_ast::ast;
use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_errors::Applicability;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext};
use rustc_session::{declare_tool_lint, impl_lint_pass};
use rustc_span::{edition::Edition, Span};

declare_clippy_lint! {
    /// **What it does:** Checks for `#[macro_use] use...`.
    ///
    /// **Why is this bad?** Since the Rust 2018 edition you can import
    /// macro's directly, this is considered idiomatic.
    ///
    /// **Known problems:** None.
    ///
    /// **Example:**
    /// ```rust
    /// #[macro_use]
    /// use lazy_static;
    /// ```
    pub MACRO_USE_IMPORTS,
    pedantic,
    "#[macro_use] is no longer needed"
}

const BRACKETS: &[char] = &['<', '>'];

/// `MacroRefData` includes the name of the macro
/// and the path from `SourceMap::span_to_filename`.
#[derive(Debug, Clone)]
pub struct MacroRefData {
    name: String,
    path: String,
}

impl MacroRefData {
    pub fn new(name: String, callee: Span, cx: &LateContext<'_, '_>) -> Self {
        let mut path = cx.sess().source_map().span_to_filename(callee).to_string();

        // std lib paths are <::std::module::file type>
        // so remove brackets, space and type.
        if path.contains('<') {
            path = path.replace(BRACKETS, "");
        }
        if path.contains(' ') {
            path = path.split(' ').next().unwrap().to_string();
        }
        Self {
            name,
            path,
        }
    }
}

#[derive(Default)]
#[allow(clippy::module_name_repetitions)]
pub struct MacroUseImports {
    /// the actual import path used and the span of the attribute above it.
    imports: Vec<(String, Span)>,
    /// the span of the macro reference, kept to ensure only one reference is used per macro call.
    collected: FxHashSet<Span>,
    mac_refs: Vec<MacroRefData>,
}

impl_lint_pass!(MacroUseImports => [MACRO_USE_IMPORTS]);

impl MacroUseImports {
    fn push_unique_macro(&mut self, cx: &LateContext<'_, '_>, span: Span) {
        let call_site = span.source_callsite();
        let name = snippet(cx, cx.sess().source_map().span_until_char(call_site, '!'), "_");
        if let Some(callee) = span.source_callee() {
            if !self.collected.contains(&call_site) {
                let name = if name.contains("::") {
                    name.split("::").last().unwrap().to_string()
                } else {
                    name.to_string()
                };
    
                self.mac_refs.push(MacroRefData::new(name, callee.def_site, cx));
                self.collected.insert(call_site);
            }
        }
    }

    fn push_unique_macro_pat_ty(&mut self, cx: &LateContext<'_, '_>, span: Span) {
        let call_site = span.source_callsite();
        let name = snippet(cx, cx.sess().source_map().span_until_char(call_site, '!'), "_");
        if let Some(callee) = span.source_callee() {
            if !self.collected.contains(&call_site) {
                self.mac_refs.push(MacroRefData::new(name.to_string(), callee.def_site, cx));
                self.collected.insert(call_site);
            }
        }
    }
}

impl<'l, 'txc> LateLintPass<'l, 'txc> for MacroUseImports {
    fn check_item(&mut self, cx: &LateContext<'_, '_>, item: &hir::Item<'_>) {
        if_chain! {
            if cx.sess().opts.edition == Edition::Edition2018;
            if let hir::ItemKind::Use(path, _kind) = &item.kind;
            if let Some(mac_attr) = item
                .attrs
                .iter()
                .find(|attr| attr.ident().map(|s| s.to_string()) == Some("macro_use".to_string()));
            if let Res::Def(DefKind::Mod, id) = path.res;
            then {
                for kid in cx.tcx.item_children(id).iter() {
                    if let Res::Def(DefKind::Macro(_mac_type), mac_id) = kid.res {
                        let span = mac_attr.span;
                        self.imports.push((cx.tcx.def_path_str(mac_id), span));
                    }
                }
            } else {
                if in_macro(item.span) {
                    self.push_unique_macro_pat_ty(cx, item.span);
                }
            }
        }
    }
    fn check_attribute(&mut self, cx: &LateContext<'_, '_>, attr: &ast::Attribute) {
        if in_macro(attr.span) {
            self.push_unique_macro(cx, attr.span);
        }
    }
    fn check_expr(&mut self, cx: &LateContext<'_, '_>, expr: &hir::Expr<'_>) {
        if in_macro(expr.span) {
            self.push_unique_macro(cx, expr.span);
        }
    }
    fn check_stmt(&mut self, cx: &LateContext<'_, '_>, stmt: &hir::Stmt<'_>) {
        if in_macro(stmt.span) {
            self.push_unique_macro(cx, stmt.span);
        }
    }
    fn check_pat(&mut self, cx: &LateContext<'_, '_>, pat: &hir::Pat<'_>) {
        if in_macro(pat.span) {
            self.push_unique_macro_pat_ty(cx, pat.span);
        }
    }
    fn check_ty(&mut self, cx: &LateContext<'_, '_>, ty: &hir::Ty<'_>) {
        if in_macro(ty.span) {
            self.push_unique_macro_pat_ty(cx, ty.span);
        }
    }

    fn check_crate_post(&mut self, cx: &LateContext<'_, '_>, _krate: &hir::Crate<'_>) {
        let mut import_map = FxHashMap::default();
        for (import, span) in &self.imports {
            let found_idx = self.mac_refs.iter().position(|mac| import.ends_with(&mac.name));
            
            if let Some(idx) = found_idx {
                let _ = self.mac_refs.remove(idx);
                proccess_macro_path(*span, import, &mut import_map);
            }
        }
        println!("{:#?}", import_map);
        let mut imports = vec![];
        for (root, rest) in import_map {
            let mut path = format!("use {}::", root);
            let mut s = None;
            let mut count = 1;
            let rest_len = rest.len();
            if rest_len > 1 {
                path.push_str("{");
            }
            for m in &rest {
                println!("{} => {:?}", root, m);
                if count == 1 {
                    s = Some(m.span());
                }
                
                let comma = if rest_len == count { "" } else { ", " };
                match m {
                    ModPath::Item { item, .. } => {
                        path.push_str(&format!("{}{}", item, comma));
                    }
                    ModPath::Nested { names, item, span } => {
                        let nested = rest.iter()
                            // filter "self" out
                            .filter(|other_m| other_m != &m)
                            // this matches the first path segment and filters non ModPath::Nested items
                            .filter(|other_m| other_m.matches(0, m))
                            .collect::<Vec<_>>();

                        println!("{:#?}", nested);

                        if nested.is_empty() {
                            path.push_str(&format!("{}::{}{}", names.join("::").to_string(), item, comma))
                        } else {
                            // use mod_a::{mod_b::{one, two}, mod_c::item, item1, item2}
                            let mod_path = if names.len() - 1 > 0 {
                                ModPath::Nested { names: names.clone(), item: item.to_string(), span: *span, }
                            } else {
                                ModPath::Item { item: names[0].to_string(), span: *span, }
                            };
                            let names = recursive_path_push(mod_path, comma, &rest, String::new());
                            path.push_str(&format!("{}::{{{}}}{}", names, item, comma))
                        }
                    }
                }
                count += 1;             
            }
            if rest_len > 1 {
                path.push_str("};");
            }
            if let Some(span) = s {
                imports.push((span, path))
            }
        }

        if !self.mac_refs.is_empty() {
            // TODO if not empty we found one we could not make a suggestion for
            // such as std::prelude::v1 or something else I haven't thought of.
            // If we defer the calling of span_lint_and_sugg we can make a decision about its
            // applicability?
        } else {
            for (span, import) in imports {
                let help = format!("use {}", import);
                span_lint_and_sugg(
                    cx,
                    MACRO_USE_IMPORTS,
                    span,
                    "`macro_use` attributes are no longer needed in the Rust 2018 edition",
                    "remove the attribute and import the macro directly, try",
                    help,
                    Applicability::MaybeIncorrect,
                )
            }
        }
    }
}

#[derive(Debug, PartialEq)]
enum ModPath {
    Item { item: String, span: Span, },
    Nested { names: Vec<String>, item: String, span: Span, },
}

impl ModPath {
    fn span(&self) -> Span {
        match self {
            Self::Item { span, .. } => *span,
            Self::Nested { span, .. } => *span,
        }
    }

    fn item(&self) -> &str {
        match self {
            Self::Item { item, .. } => item,
            Self::Nested { item, .. } => item,
        }
    }

    fn matches(&self, idx: usize, other: &ModPath) -> bool {
        match (self, other) {
            (Self::Item { item, .. }, Self::Item { item: other_item, .. }) => item == other_item,
            (Self::Nested { names, .. }, Self::Nested { names: other_names, .. }) => {
                match (names.get(idx), other_names.get(idx)) {
                    (Some(seg), Some(other_seg)) => seg == other_seg,
                    (_, _) => false,
                }
            }
            (_, _) => false,
        }
    }
}

fn proccess_macro_path(span: Span, import: &str, import_map: &mut FxHashMap<String, Vec<ModPath>>) {
    let mut mod_path = import.split("::").collect::<Vec<_>>();

    if mod_path.len() == 2 {
        let item_list = import_map.entry(mod_path[0].to_string())
            .or_insert(vec![]);

        if !item_list.iter().any(|mods| mods.item() == mod_path[1]) {
            item_list.push(ModPath::Item{
                item: mod_path[1].to_string(),
                span,
            });
        }
    } else if mod_path.len() > 2 {
        let first = mod_path.remove(0);
        let name = mod_path.remove(mod_path.len() - 1);

        let nested = ModPath::Nested {
            names: mod_path.into_iter().map(ToString::to_string).collect(),
            item: name.to_string(),
            span,
        };
        import_map.entry(first.to_string())
            .or_insert(vec![])
            .push(nested);
    } else {
        unreachable!("test to see if code path hit TODO REMOVE")
    }
}

fn recursive_path_push(module: ModPath, comma: &str, rest: &[ModPath], mut path: String) -> String {
    match &module {
        ModPath::Item { item, .. } => {
            path.push_str(&format!("{}{}", item, comma));
        }
        ModPath::Nested { names, item, span } => {
            let nested = rest.iter()
                // filter "self" out
                .filter(|other_m| other_m != &&module)
                // this matches the first path segment and filters non ModPath::Nested items
                .filter(|other_m| other_m.matches(0, &module))
                .collect::<Vec<_>>();

            println!("{:#?}", nested);

            if nested.is_empty() {
                path.push_str(&format!("{}::{}{}", names.join("::").to_string(), item, comma))
            } else {
                // use mod_a::{mod_b::{one, two}, mod_c::item, item1, item2}
                let mod_path = if names.len() - 1 > 0 {
                    ModPath::Nested { names: names.clone(), item: item.to_string(), span: *span, }
                } else {
                    ModPath::Item { item: names[0].to_string(), span: *span, }
                };
                let names = recursive_path_push(mod_path, comma, rest, path.to_string());
                // path.push_str(&format!("{}{}", item, comma));
            }
        }
    }
    path
}

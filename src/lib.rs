#![feature(type_macros)]
#![feature(plugin_registrar, box_syntax)]
#![feature(rustc_private, collections)]
#![feature(iter_arith)]
#![feature(custom_attribute)]
#![feature(slice_patterns)]
#![feature(question_mark)]
#![feature(stmt_expr_attributes)]
#![allow(indexing_slicing, shadow_reuse, unknown_lints)]

extern crate rustc_driver;
extern crate getopts;

use rustc_driver::{driver, CompilerCalls, RustcDefaultCalls, Compilation};
use rustc::session::{config, Session};
use rustc::session::config::{Input, ErrorOutputType};
use syntax::diagnostics;
use std::path::PathBuf;
use std::process::Command;

struct ClippyCompilerCalls(RustcDefaultCalls);

impl std::default::Default for ClippyCompilerCalls {
    fn default() -> Self {
        Self::new()
    }
}

impl ClippyCompilerCalls {
    fn new() -> Self {
        ClippyCompilerCalls(RustcDefaultCalls)
    }
}

impl<'a> CompilerCalls<'a> for ClippyCompilerCalls {
    fn early_callback(&mut self,
                      matches: &getopts::Matches,
                      sopts: &config::Options,
                      descriptions: &diagnostics::registry::Registry,
                      output: ErrorOutputType)
                      -> Compilation {
        self.0.early_callback(matches, sopts, descriptions, output)
    }
    fn no_input(&mut self,
                matches: &getopts::Matches,
                sopts: &config::Options,
                odir: &Option<PathBuf>,
                ofile: &Option<PathBuf>,
                descriptions: &diagnostics::registry::Registry)
                -> Option<(Input, Option<PathBuf>)> {
        self.0.no_input(matches, sopts, odir, ofile, descriptions)
    }
    fn late_callback(&mut self,
                     matches: &getopts::Matches,
                     sess: &Session,
                     input: &Input,
                     odir: &Option<PathBuf>,
                     ofile: &Option<PathBuf>)
                     -> Compilation {
        self.0.late_callback(matches, sess, input, odir, ofile)
    }
    fn build_controller(&mut self, sess: &Session, matches: &getopts::Matches) -> driver::CompileController<'a> {
        let mut control = self.0.build_controller(sess, matches);

        let old = std::mem::replace(&mut control.after_parse.callback, box |_| {});
        control.after_parse.callback = Box::new(move |state| {
            {
                let mut registry = rustc_plugin::registry::Registry::new(state.session, state.krate.as_ref().expect("at this compilation stage the krate must be parsed"));
                registry.args_hidden = Some(Vec::new());
                plugin_registrar(&mut registry);

                let rustc_plugin::registry::Registry { early_lint_passes, late_lint_passes, lint_groups, llvm_passes, attributes, mir_passes, .. } = registry;
                let sess = &state.session;
                let mut ls = sess.lint_store.borrow_mut();
                for pass in early_lint_passes {
                    ls.register_early_pass(Some(sess), true, pass);
                }
                for pass in late_lint_passes {
                    ls.register_late_pass(Some(sess), true, pass);
                }

                for (name, to) in lint_groups {
                    ls.register_group(Some(sess), true, name, to);
                }

                sess.plugin_llvm_passes.borrow_mut().extend(llvm_passes);
                sess.mir_passes.borrow_mut().extend(mir_passes);
                sess.plugin_attributes.borrow_mut().extend(attributes);
            }
            old(state);
        });

        control
    }
}

use std::path::Path;

pub fn main() {
    use std::env;

    if env::var("CLIPPY_DOGFOOD").map(|_| true).unwrap_or(false) {
        return;
    }

    let dep_path = env::current_dir().expect("current dir is not readable").join("target").join("debug").join("deps");

    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    let sys_root = match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("SYSROOT").map(|s| s.to_owned())
                                   .or(Command::new("rustc").arg("--print")
                                                            .arg("sysroot")
                                                            .output().ok()
                                                            .and_then(|out| String::from_utf8(out.stdout).ok())
                                                            .map(|s| s.trim().to_owned())
                                                            )
                .expect("need to specify SYSROOT env var during clippy compilation, or use rustup or multirust"),
    };

    if let Some("clippy") = std::env::args().nth(1).as_ref().map(AsRef::as_ref) {
        let args = wrap_args(std::env::args().skip(2), dep_path, sys_root);
        let path = std::env::current_exe().expect("current executable path invalid");
        let exit_status = std::process::Command::new("cargo")
            .args(&args)
            .env("RUSTC", path)
            .spawn().expect("could not run cargo")
            .wait().expect("failed to wait for cargo?");

        if let Some(code) = exit_status.code() {
            std::process::exit(code);
        }
    } else {
        let args: Vec<String> = if env::args().any(|s| s == "--sysroot") {
            env::args().collect()
        } else {
            env::args().chain(Some("--sysroot".to_owned())).chain(Some(sys_root)).collect()
        };
        let (result, _) = rustc_driver::run_compiler(&args, &mut ClippyCompilerCalls::new());

        if let Err(err_count) = result {
            if err_count > 0 {
                std::process::exit(1);
            }
        }
    }
}

fn wrap_args<P, I>(old_args: I, dep_path: P, sysroot: String) -> Vec<String>
    where P: AsRef<Path>, I: Iterator<Item=String> {

    let mut args = vec!["rustc".to_owned()];

    let mut found_dashes = false;
    for arg in old_args {
        found_dashes |= arg == "--";
        args.push(arg);
    }
    if !found_dashes {
        args.push("--".to_owned());
    }
    args.push("-L".to_owned());
    args.push(dep_path.as_ref().to_string_lossy().into_owned());
    args.push(String::from("--sysroot"));
    args.push(sysroot);
    args.push("-Zno-trans".to_owned());
    args
}

#[macro_use]
extern crate syntax;
#[macro_use]
extern crate rustc;

extern crate toml;

// Only for the compile time checking of paths
extern crate core;
extern crate collections;

// for unicode nfc normalization
extern crate unicode_normalization;

// for semver check in attrs.rs
extern crate semver;

// for regex checking
extern crate regex_syntax;

// for finding minimal boolean expressions
extern crate quine_mc_cluskey;

extern crate rustc_plugin;
extern crate rustc_const_eval;
extern crate rustc_const_math;
use rustc_plugin::Registry;

macro_rules! declare_restriction_lint {
    { pub $name:tt, $description:tt } => {
        declare_lint! { pub $name, Allow, $description }
    };
}

pub mod consts;
#[macro_use]
pub mod utils;

// begin lints modules, do not remove this comment, it’s used in `update_lints`
pub mod approx_const;
pub mod arithmetic;
pub mod array_indexing;
pub mod assign_ops;
pub mod attrs;
pub mod bit_mask;
pub mod blacklisted_name;
pub mod block_in_if_condition;
pub mod booleans;
pub mod collapsible_if;
pub mod copies;
pub mod cyclomatic_complexity;
pub mod derive;
pub mod doc;
pub mod drop_ref;
pub mod entry;
pub mod enum_clike;
pub mod enum_glob_use;
pub mod enum_variants;
pub mod eq_op;
pub mod escape;
pub mod eta_reduction;
pub mod format;
pub mod formatting;
pub mod functions;
pub mod identity_op;
pub mod if_not_else;
pub mod items_after_statements;
pub mod len_zero;
pub mod lifetimes;
pub mod loops;
pub mod map_clone;
pub mod matches;
pub mod mem_forget;
pub mod methods;
pub mod minmax;
pub mod misc;
pub mod misc_early;
pub mod mut_mut;
pub mod mut_reference;
pub mod mutex_atomic;
pub mod needless_bool;
pub mod needless_borrow;
pub mod needless_update;
pub mod neg_multiply;
pub mod new_without_default;
pub mod no_effect;
pub mod non_expressive_names;
pub mod open_options;
pub mod overflow_check_conditional;
pub mod panic;
pub mod precedence;
pub mod print;
pub mod ptr_arg;
pub mod ranges;
pub mod regex;
pub mod returns;
pub mod shadow;
pub mod strings;
pub mod swap;
pub mod temporary_assignment;
pub mod transmute;
pub mod types;
pub mod unicode;
pub mod unsafe_removed_from_name;
pub mod unused_label;
pub mod vec;
pub mod zero_div_zero;
// end lints modules, do not remove this comment, it’s used in `update_lints`

mod reexport {
    pub use syntax::ast::{Name, NodeId};
}

#[plugin_registrar]
#[cfg_attr(rustfmt, rustfmt_skip)]
pub fn plugin_registrar(reg: &mut Registry) {
    let conf = match utils::conf::conf_file(reg.args()) {
        Ok(file_name) => {
            // if the user specified a file, it must exist, otherwise default to `clippy.toml` but
            // do not require the file to exist
            let (ref file_name, must_exist) = if let Some(ref file_name) = file_name {
                (&**file_name, true)
            } else {
                ("clippy.toml", false)
            };

            let (conf, errors) = utils::conf::read_conf(file_name, must_exist);

            // all conf errors are non-fatal, we just use the default conf in case of error
            for error in errors {
                reg.sess.struct_err(&format!("error reading Clippy's configuration file: {}", error)).emit();
            }

            conf
        }
        Err((err, span)) => {
            reg.sess.struct_span_err(span, err)
                    .span_note(span, "Clippy will use default configuration")
                    .emit();
            utils::conf::Conf::default()
        }
    };

    let mut store = reg.sess.lint_store.borrow_mut();
    store.register_removed("unstable_as_slice", "`Vec::as_slice` has been stabilized in 1.7");
    store.register_removed("unstable_as_mut_slice", "`Vec::as_mut_slice` has been stabilized in 1.7");
    store.register_removed("str_to_string", "using `str::to_string` is common even today and specialization will likely happen soon");
    store.register_removed("string_to_string", "using `string::to_string` is common even today and specialization will likely happen soon");
    // end deprecated lints, do not remove this comment, it’s used in `update_lints`

    reg.register_late_lint_pass(box types::TypePass);
    reg.register_late_lint_pass(box booleans::NonminimalBool);
    reg.register_late_lint_pass(box misc::TopLevelRefPass);
    reg.register_late_lint_pass(box misc::CmpNan);
    reg.register_late_lint_pass(box eq_op::EqOp);
    reg.register_early_lint_pass(box enum_variants::EnumVariantNames);
    reg.register_late_lint_pass(box enum_glob_use::EnumGlobUse);
    reg.register_late_lint_pass(box enum_clike::EnumClikeUnportableVariant);
    reg.register_late_lint_pass(box bit_mask::BitMask);
    reg.register_late_lint_pass(box ptr_arg::PtrArg);
    reg.register_late_lint_pass(box needless_bool::NeedlessBool);
    reg.register_late_lint_pass(box needless_bool::BoolComparison);
    reg.register_late_lint_pass(box approx_const::ApproxConstant);
    reg.register_late_lint_pass(box misc::FloatCmp);
    reg.register_early_lint_pass(box precedence::Precedence);
    reg.register_late_lint_pass(box eta_reduction::EtaPass);
    reg.register_late_lint_pass(box identity_op::IdentityOp);
    reg.register_early_lint_pass(box items_after_statements::ItemsAfterStatements);
    reg.register_late_lint_pass(box mut_mut::MutMut);
    reg.register_late_lint_pass(box mut_reference::UnnecessaryMutPassed);
    reg.register_late_lint_pass(box len_zero::LenZero);
    reg.register_late_lint_pass(box misc::CmpOwned);
    reg.register_late_lint_pass(box attrs::AttrPass);
    reg.register_late_lint_pass(box collapsible_if::CollapsibleIf);
    reg.register_late_lint_pass(box block_in_if_condition::BlockInIfCondition);
    reg.register_late_lint_pass(box misc::ModuloOne);
    reg.register_late_lint_pass(box unicode::Unicode);
    reg.register_late_lint_pass(box strings::StringAdd);
    reg.register_early_lint_pass(box returns::ReturnPass);
    reg.register_late_lint_pass(box methods::MethodsPass);
    reg.register_late_lint_pass(box shadow::ShadowPass);
    reg.register_late_lint_pass(box types::LetPass);
    reg.register_late_lint_pass(box types::UnitCmp);
    reg.register_late_lint_pass(box loops::LoopsPass);
    reg.register_late_lint_pass(box lifetimes::LifetimePass);
    reg.register_late_lint_pass(box entry::HashMapLint);
    reg.register_late_lint_pass(box ranges::StepByZero);
    reg.register_late_lint_pass(box types::CastPass);
    reg.register_late_lint_pass(box types::TypeComplexityPass::new(conf.type_complexity_threshold));
    reg.register_late_lint_pass(box matches::MatchPass);
    reg.register_late_lint_pass(box misc::PatternPass);
    reg.register_late_lint_pass(box minmax::MinMaxPass);
    reg.register_late_lint_pass(box open_options::NonSensicalOpenOptions);
    reg.register_late_lint_pass(box zero_div_zero::ZeroDivZeroPass);
    reg.register_late_lint_pass(box mutex_atomic::MutexAtomic);
    reg.register_late_lint_pass(box needless_update::NeedlessUpdatePass);
    reg.register_late_lint_pass(box needless_borrow::NeedlessBorrow);
    reg.register_late_lint_pass(box no_effect::NoEffectPass);
    reg.register_late_lint_pass(box map_clone::MapClonePass);
    reg.register_late_lint_pass(box temporary_assignment::TemporaryAssignmentPass);
    reg.register_late_lint_pass(box transmute::Transmute);
    reg.register_late_lint_pass(box cyclomatic_complexity::CyclomaticComplexity::new(conf.cyclomatic_complexity_threshold));
    reg.register_late_lint_pass(box escape::EscapePass);
    reg.register_early_lint_pass(box misc_early::MiscEarly);
    reg.register_late_lint_pass(box misc::UsedUnderscoreBinding);
    reg.register_late_lint_pass(box array_indexing::ArrayIndexing);
    reg.register_late_lint_pass(box panic::PanicPass);
    reg.register_late_lint_pass(box strings::StringLitAsBytes);
    reg.register_late_lint_pass(box derive::Derive);
    reg.register_late_lint_pass(box types::CharLitAsU8);
    reg.register_late_lint_pass(box print::PrintLint);
    reg.register_late_lint_pass(box vec::UselessVec);
    reg.register_early_lint_pass(box non_expressive_names::NonExpressiveNames {
        max_single_char_names: conf.max_single_char_names,
    });
    reg.register_late_lint_pass(box drop_ref::DropRefPass);
    reg.register_late_lint_pass(box types::AbsurdExtremeComparisons);
    reg.register_late_lint_pass(box types::InvalidUpcastComparisons);
    reg.register_late_lint_pass(box regex::RegexPass::default());
    reg.register_late_lint_pass(box copies::CopyAndPaste);
    reg.register_late_lint_pass(box format::FormatMacLint);
    reg.register_early_lint_pass(box formatting::Formatting);
    reg.register_late_lint_pass(box swap::Swap);
    reg.register_early_lint_pass(box if_not_else::IfNotElse);
    reg.register_late_lint_pass(box overflow_check_conditional::OverflowCheckConditional);
    reg.register_late_lint_pass(box unused_label::UnusedLabel);
    reg.register_late_lint_pass(box new_without_default::NewWithoutDefault);
    reg.register_late_lint_pass(box blacklisted_name::BlackListedName::new(conf.blacklisted_names));
    reg.register_late_lint_pass(box functions::Functions::new(conf.too_many_arguments_threshold));
    reg.register_early_lint_pass(box doc::Doc::new(conf.doc_valid_idents));
    reg.register_late_lint_pass(box neg_multiply::NegMultiply);
    reg.register_late_lint_pass(box unsafe_removed_from_name::UnsafeNameRemoval);
    reg.register_late_lint_pass(box mem_forget::MemForget);
    reg.register_late_lint_pass(box arithmetic::Arithmetic::default());
    reg.register_late_lint_pass(box assign_ops::AssignOps);

    reg.register_lint_group("clippy_restrictions", vec![
        arithmetic::FLOAT_ARITHMETIC,
        arithmetic::INTEGER_ARITHMETIC,
        assign_ops::ASSIGN_OPS,
    ]);

    reg.register_lint_group("clippy_pedantic", vec![
        array_indexing::INDEXING_SLICING,
        booleans::NONMINIMAL_BOOL,
        enum_glob_use::ENUM_GLOB_USE,
        if_not_else::IF_NOT_ELSE,
        items_after_statements::ITEMS_AFTER_STATEMENTS,
        matches::SINGLE_MATCH_ELSE,
        mem_forget::MEM_FORGET,
        methods::OPTION_UNWRAP_USED,
        methods::RESULT_UNWRAP_USED,
        methods::WRONG_PUB_SELF_CONVENTION,
        misc::USED_UNDERSCORE_BINDING,
        mut_mut::MUT_MUT,
        mutex_atomic::MUTEX_INTEGER,
        non_expressive_names::SIMILAR_NAMES,
        print::PRINT_STDOUT,
        print::USE_DEBUG,
        shadow::SHADOW_REUSE,
        shadow::SHADOW_SAME,
        shadow::SHADOW_UNRELATED,
        strings::STRING_ADD,
        strings::STRING_ADD_ASSIGN,
        types::CAST_POSSIBLE_TRUNCATION,
        types::CAST_POSSIBLE_WRAP,
        types::CAST_PRECISION_LOSS,
        types::CAST_SIGN_LOSS,
        types::INVALID_UPCAST_COMPARISONS,
        unicode::NON_ASCII_LITERAL,
        unicode::UNICODE_NOT_NFC,
    ]);

    reg.register_lint_group("clippy", vec![
        approx_const::APPROX_CONSTANT,
        array_indexing::OUT_OF_BOUNDS_INDEXING,
        assign_ops::ASSIGN_OP_PATTERN,
        attrs::DEPRECATED_SEMVER,
        attrs::INLINE_ALWAYS,
        bit_mask::BAD_BIT_MASK,
        bit_mask::INEFFECTIVE_BIT_MASK,
        blacklisted_name::BLACKLISTED_NAME,
        block_in_if_condition::BLOCK_IN_IF_CONDITION_EXPR,
        block_in_if_condition::BLOCK_IN_IF_CONDITION_STMT,
        booleans::LOGIC_BUG,
        collapsible_if::COLLAPSIBLE_IF,
        copies::IF_SAME_THEN_ELSE,
        copies::IFS_SAME_COND,
        copies::MATCH_SAME_ARMS,
        cyclomatic_complexity::CYCLOMATIC_COMPLEXITY,
        derive::DERIVE_HASH_XOR_EQ,
        derive::EXPL_IMPL_CLONE_ON_COPY,
        doc::DOC_MARKDOWN,
        drop_ref::DROP_REF,
        entry::MAP_ENTRY,
        enum_clike::ENUM_CLIKE_UNPORTABLE_VARIANT,
        enum_variants::ENUM_VARIANT_NAMES,
        eq_op::EQ_OP,
        escape::BOXED_LOCAL,
        eta_reduction::REDUNDANT_CLOSURE,
        format::USELESS_FORMAT,
        formatting::SUSPICIOUS_ASSIGNMENT_FORMATTING,
        formatting::SUSPICIOUS_ELSE_FORMATTING,
        functions::TOO_MANY_ARGUMENTS,
        identity_op::IDENTITY_OP,
        len_zero::LEN_WITHOUT_IS_EMPTY,
        len_zero::LEN_ZERO,
        lifetimes::NEEDLESS_LIFETIMES,
        lifetimes::UNUSED_LIFETIMES,
        loops::EMPTY_LOOP,
        loops::EXPLICIT_COUNTER_LOOP,
        loops::EXPLICIT_ITER_LOOP,
        loops::FOR_KV_MAP,
        loops::FOR_LOOP_OVER_OPTION,
        loops::FOR_LOOP_OVER_RESULT,
        loops::ITER_NEXT_LOOP,
        loops::NEEDLESS_RANGE_LOOP,
        loops::REVERSE_RANGE_LOOP,
        loops::UNUSED_COLLECT,
        loops::WHILE_LET_LOOP,
        loops::WHILE_LET_ON_ITERATOR,
        map_clone::MAP_CLONE,
        matches::MATCH_BOOL,
        matches::MATCH_OVERLAPPING_ARM,
        matches::MATCH_REF_PATS,
        matches::SINGLE_MATCH,
        methods::CHARS_NEXT_CMP,
        methods::CLONE_DOUBLE_REF,
        methods::CLONE_ON_COPY,
        methods::EXTEND_FROM_SLICE,
        methods::FILTER_NEXT,
        methods::NEW_RET_NO_SELF,
        methods::OK_EXPECT,
        methods::OPTION_MAP_UNWRAP_OR,
        methods::OPTION_MAP_UNWRAP_OR_ELSE,
        methods::OR_FUN_CALL,
        methods::SEARCH_IS_SOME,
        methods::SHOULD_IMPLEMENT_TRAIT,
        methods::SINGLE_CHAR_PATTERN,
        methods::TEMPORARY_CSTRING_AS_PTR,
        methods::WRONG_SELF_CONVENTION,
        minmax::MIN_MAX,
        misc::CMP_NAN,
        misc::CMP_OWNED,
        misc::FLOAT_CMP,
        misc::MODULO_ONE,
        misc::REDUNDANT_PATTERN,
        misc::TOPLEVEL_REF_ARG,
        misc_early::DUPLICATE_UNDERSCORE_ARGUMENT,
        misc_early::REDUNDANT_CLOSURE_CALL,
        misc_early::UNNEEDED_FIELD_PATTERN,
        mut_reference::UNNECESSARY_MUT_PASSED,
        mutex_atomic::MUTEX_ATOMIC,
        needless_bool::BOOL_COMPARISON,
        needless_bool::NEEDLESS_BOOL,
        needless_borrow::NEEDLESS_BORROW,
        needless_update::NEEDLESS_UPDATE,
        neg_multiply::NEG_MULTIPLY,
        new_without_default::NEW_WITHOUT_DEFAULT,
        new_without_default::NEW_WITHOUT_DEFAULT_DERIVE,
        no_effect::NO_EFFECT,
        no_effect::UNNECESSARY_OPERATION,
        non_expressive_names::MANY_SINGLE_CHAR_NAMES,
        open_options::NONSENSICAL_OPEN_OPTIONS,
        overflow_check_conditional::OVERFLOW_CHECK_CONDITIONAL,
        panic::PANIC_PARAMS,
        precedence::PRECEDENCE,
        ptr_arg::PTR_ARG,
        ranges::RANGE_STEP_BY_ZERO,
        ranges::RANGE_ZIP_WITH_LEN,
        regex::INVALID_REGEX,
        regex::REGEX_MACRO,
        regex::TRIVIAL_REGEX,
        returns::LET_AND_RETURN,
        returns::NEEDLESS_RETURN,
        strings::STRING_LIT_AS_BYTES,
        swap::ALMOST_SWAPPED,
        swap::MANUAL_SWAP,
        temporary_assignment::TEMPORARY_ASSIGNMENT,
        transmute::CROSSPOINTER_TRANSMUTE,
        transmute::TRANSMUTE_PTR_TO_REF,
        transmute::USELESS_TRANSMUTE,
        types::ABSURD_EXTREME_COMPARISONS,
        types::BOX_VEC,
        types::CHAR_LIT_AS_U8,
        types::LET_UNIT_VALUE,
        types::LINKEDLIST,
        types::TYPE_COMPLEXITY,
        types::UNIT_CMP,
        unicode::ZERO_WIDTH_SPACE,
        unsafe_removed_from_name::UNSAFE_REMOVED_FROM_NAME,
        unused_label::UNUSED_LABEL,
        vec::USELESS_VEC,
        zero_div_zero::ZERO_DIVIDED_BY_ZERO,
    ]);
}

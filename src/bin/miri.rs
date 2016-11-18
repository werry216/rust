#![feature(rustc_private)]

extern crate getopts;
extern crate miri;
extern crate rustc;
extern crate rustc_driver;
extern crate env_logger;
extern crate log_settings;
extern crate syntax;
#[macro_use] extern crate log;

use miri::{eval_main, run_mir_passes};
use rustc::session::Session;
use rustc_driver::{driver, CompilerCalls, Compilation};
use syntax::ast::{MetaItemKind, NestedMetaItemKind};

struct MiriCompilerCalls;

impl<'a> CompilerCalls<'a> for MiriCompilerCalls {
    fn build_controller(
        &mut self,
        _: &Session,
        _: &getopts::Matches
    ) -> driver::CompileController<'a> {
        let mut control = driver::CompileController::basic();
        control.after_hir_lowering.callback = Box::new(|state| {
            state.session.plugin_attributes.borrow_mut().push(("miri".to_owned(), syntax::feature_gate::AttributeType::Whitelisted));
        });
        control.after_analysis.stop = Compilation::Stop;
        control.after_analysis.callback = Box::new(|state| {
            state.session.abort_if_errors();

            let tcx = state.tcx.unwrap();
            let (entry_node_id, _) = state.session.entry_fn.borrow()
                .expect("no main or start function found");
            let entry_def_id = tcx.map.local_def_id(entry_node_id);

            let krate = state.hir_crate.as_ref().unwrap();
            let mut memory_size = 100*1024*1024; // 100MB
            let mut step_limit = 1000_000;
            let mut stack_limit = 100;
            let extract_int = |lit: &syntax::ast::Lit| -> u64 {
                match lit.node {
                    syntax::ast::LitKind::Int(i, _) => i,
                    _ => state.session.span_fatal(lit.span, "expected an integer literal"),
                }
            };
            for attr in krate.attrs.iter() {
                match attr.node.value.node {
                    MetaItemKind::List(ref name, _) if name != "miri" => {}
                    MetaItemKind::List(_, ref items) => for item in items {
                        match item.node {
                            NestedMetaItemKind::MetaItem(ref inner) => match inner.node {
                                MetaItemKind::NameValue(ref name, ref value) => {
                                    match &**name {
                                        "memory_size" => memory_size = extract_int(value),
                                        "step_limit" => step_limit = extract_int(value),
                                        "stack_limit" => stack_limit = extract_int(value) as usize,
                                        _ => state.session.span_err(item.span, "unknown miri attribute"),
                                    }
                                }
                                _ => state.session.span_err(inner.span, "miri attributes need to be of key = value kind"),
                            },
                            _ => state.session.span_err(item.span, "miri attributes need to be of key = value kind"),
                        }
                    },
                    _ => {},
                }
            }

            run_mir_passes(tcx);
            eval_main(tcx, entry_def_id, memory_size, step_limit, stack_limit);

            state.session.abort_if_errors();
        });

        control
    }
}

fn init_logger() {
    const MAX_INDENT: usize = 40;

    let format = |record: &log::LogRecord| {
        if record.level() == log::LogLevel::Trace {
            // prepend spaces to indent the final string
            let indentation = log_settings::settings().indentation;
            format!("{lvl}:{module}{depth:2}{indent:<indentation$} {text}",
                lvl = record.level(),
                module = record.location().module_path(),
                depth = indentation / MAX_INDENT,
                indentation = indentation % MAX_INDENT,
                indent = "",
                text = record.args())
        } else {
            format!("{lvl}:{module}: {text}",
                lvl = record.level(),
                module = record.location().module_path(),
                text = record.args())
        }
    };

    let mut builder = env_logger::LogBuilder::new();
    builder.format(format).filter(None, log::LogLevelFilter::Info);

    if std::env::var("MIRI_LOG").is_ok() {
        builder.parse(&std::env::var("MIRI_LOG").unwrap());
    }

    builder.init().unwrap();
}

fn find_sysroot() -> String {
    // Taken from https://github.com/Manishearth/rust-clippy/pull/911.
    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("RUST_SYSROOT")
            .expect("need to specify RUST_SYSROOT env var or use rustup or multirust")
            .to_owned(),
    }
}

fn main() {
    init_logger();
    let mut args: Vec<String> = std::env::args().collect();

    let sysroot_flag = String::from("--sysroot");
    if !args.contains(&sysroot_flag) {
        args.push(sysroot_flag);
        args.push(find_sysroot());
    }

    rustc_driver::run_compiler(&args, &mut MiriCompilerCalls, None, None);
}

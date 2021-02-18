//! Support code for rustc's built in unit-test and micro-benchmarking
//! framework.
//!
//! Almost all user code will only be interested in `Bencher` and
//! `black_box`. All other interactions (such as writing tests and
//! benchmarks themselves) should be done via the `#[test]` and
//! `#[bench]` attributes.
//!
//! See the [Testing Chapter](../book/ch11-00-testing.html) of the book for more details.

// Currently, not much of this is meant for users. It is intended to
// support the simplest interface possible for representing and
// running tests while providing a base that other test frameworks may
// build off of.

// N.B., this is also specified in this crate's Cargo.toml, but librustc_ast contains logic specific to
// this crate, which relies on this attribute (rather than the value of `--crate-name` passed by
// cargo) to detect this crate.

#![crate_name = "test"]
#![unstable(feature = "test", issue = "50297")]
#![doc(html_root_url = "https://doc.rust-lang.org/nightly/", test(attr(deny(warnings))))]
#![cfg_attr(unix, feature(libc))]
#![feature(rustc_private)]
#![feature(nll)]
#![feature(available_concurrency)]
#![feature(internal_output_capture)]
#![feature(option_unwrap_none)]
#![feature(panic_unwind)]
#![feature(staged_api)]
#![feature(termination_trait_lib)]
#![feature(test)]
#![feature(total_cmp)]
#![feature(str_split_once)]

// Public reexports
pub use self::bench::{black_box, Bencher};
pub use self::console::run_tests_console;
pub use self::options::{ColorConfig, Options, OutputFormat, RunIgnored, ShouldPanic};
pub use self::types::TestName::*;
pub use self::types::*;
pub use self::ColorConfig::*;
pub use cli::TestOpts;

// Module to be used by rustc to compile tests in libtest
pub mod test {
    pub use crate::{
        assert_test_result,
        bench::Bencher,
        cli::{parse_opts, TestOpts},
        filter_tests,
        helpers::metrics::{Metric, MetricMap},
        options::{Options, RunIgnored, RunStrategy, ShouldPanic},
        run_test, test_main, test_main_static,
        test_result::{TestResult, TrFailed, TrFailedMsg, TrIgnored, TrOk},
        time::{TestExecTime, TestTimeOptions},
        types::{
            DynTestFn, DynTestName, StaticBenchFn, StaticTestFn, StaticTestName, TestDesc,
            TestDescAndFn, TestName, TestType,
        },
    };
}

use std::{
    collections::VecDeque,
    env, io,
    io::prelude::Write,
    panic::{self, catch_unwind, AssertUnwindSafe, PanicInfo},
    process::{self, Command, Termination},
    sync::mpsc::{channel, Sender},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

pub mod bench;
mod cli;
mod console;
mod event;
mod formatters;
mod helpers;
mod options;
pub mod stats;
mod test_result;
mod time;
mod types;

#[cfg(test)]
mod tests;

use event::{CompletedTest, TestEvent};
use helpers::concurrency::get_concurrency;
use helpers::exit_code::get_exit_code;
use options::{Concurrent, RunStrategy};
use test_result::*;
use time::TestExecTime;

// Process exit code to be used to indicate test failures.
const ERROR_EXIT_CODE: i32 = 101;

const SECONDARY_TEST_INVOKER_VAR: &str = "__RUST_TEST_INVOKE";

// The default console test runner. It accepts the command line
// arguments and a vector of test_descs.
pub fn test_main(args: &[String], tests: Vec<TestDescAndFn>, options: Option<Options>) {
    let mut opts = match cli::parse_opts(args) {
        Some(Ok(o)) => o,
        Some(Err(msg)) => {
            eprintln!("error: {}", msg);
            process::exit(ERROR_EXIT_CODE);
        }
        None => return,
    };
    if let Some(options) = options {
        opts.options = options;
    }
    if opts.list {
        if let Err(e) = console::list_tests_console(&opts, tests) {
            eprintln!("error: io error when listing tests: {:?}", e);
            process::exit(ERROR_EXIT_CODE);
        }
    } else {
        match console::run_tests_console(&opts, tests) {
            Ok(true) => {}
            Ok(false) => process::exit(ERROR_EXIT_CODE),
            Err(e) => {
                eprintln!("error: io error when listing tests: {:?}", e);
                process::exit(ERROR_EXIT_CODE);
            }
        }
    }
}

/// A variant optimized for invocation with a static test vector.
/// This will panic (intentionally) when fed any dynamic tests.
///
/// This is the entry point for the main function generated by `rustc --test`
/// when panic=unwind.
pub fn test_main_static(tests: &[&TestDescAndFn]) {
    let args = env::args().collect::<Vec<_>>();
    let owned_tests: Vec<_> = tests.iter().map(make_owned_test).collect();
    test_main(&args, owned_tests, None)
}

/// A variant optimized for invocation with a static test vector.
/// This will panic (intentionally) when fed any dynamic tests.
///
/// Runs tests in panic=abort mode, which involves spawning subprocesses for
/// tests.
///
/// This is the entry point for the main function generated by `rustc --test`
/// when panic=abort.
pub fn test_main_static_abort(tests: &[&TestDescAndFn]) {
    // If we're being run in SpawnedSecondary mode, run the test here. run_test
    // will then exit the process.
    if let Ok(name) = env::var(SECONDARY_TEST_INVOKER_VAR) {
        env::remove_var(SECONDARY_TEST_INVOKER_VAR);
        let test = tests
            .iter()
            .filter(|test| test.desc.name.as_slice() == name)
            .map(make_owned_test)
            .next()
            .unwrap_or_else(|| panic!("couldn't find a test with the provided name '{}'", name));
        let TestDescAndFn { desc, testfn } = test;
        let testfn = match testfn {
            StaticTestFn(f) => f,
            _ => panic!("only static tests are supported"),
        };
        run_test_in_spawned_subprocess(desc, Box::new(testfn));
    }

    let args = env::args().collect::<Vec<_>>();
    let owned_tests: Vec<_> = tests.iter().map(make_owned_test).collect();
    test_main(&args, owned_tests, Some(Options::new().panic_abort(true)))
}

/// Clones static values for putting into a dynamic vector, which test_main()
/// needs to hand out ownership of tests to parallel test runners.
///
/// This will panic when fed any dynamic tests, because they cannot be cloned.
fn make_owned_test(test: &&TestDescAndFn) -> TestDescAndFn {
    match test.testfn {
        StaticTestFn(f) => TestDescAndFn { testfn: StaticTestFn(f), desc: test.desc.clone() },
        StaticBenchFn(f) => TestDescAndFn { testfn: StaticBenchFn(f), desc: test.desc.clone() },
        _ => panic!("non-static tests passed to test::test_main_static"),
    }
}

/// Invoked when unit tests terminate. Should panic if the unit
/// Tests is considered a failure. By default, invokes `report()`
/// and checks for a `0` result.
pub fn assert_test_result<T: Termination>(result: T) {
    let code = result.report();
    assert_eq!(
        code, 0,
        "the test returned a termination value with a non-zero status code ({}) \
         which indicates a failure",
        code
    );
}

pub fn run_tests<F>(
    opts: &TestOpts,
    tests: Vec<TestDescAndFn>,
    mut notify_about_test_event: F,
) -> io::Result<()>
where
    F: FnMut(TestEvent) -> io::Result<()>,
{
    use std::collections::{self, HashMap};
    use std::hash::BuildHasherDefault;
    use std::sync::mpsc::RecvTimeoutError;

    struct RunningTest {
        join_handle: Option<thread::JoinHandle<()>>,
    }

    // Use a deterministic hasher
    type TestMap =
        HashMap<TestDesc, RunningTest, BuildHasherDefault<collections::hash_map::DefaultHasher>>;

    struct TimeoutEntry {
        desc: TestDesc,
        timeout: Instant,
    }

    let tests_len = tests.len();

    let mut filtered_tests = filter_tests(opts, tests);
    if !opts.bench_benchmarks {
        filtered_tests = convert_benchmarks_to_tests(filtered_tests);
    }

    let filtered_tests = {
        let mut filtered_tests = filtered_tests;
        for test in filtered_tests.iter_mut() {
            test.desc.name = test.desc.name.with_padding(test.testfn.padding());
        }

        filtered_tests
    };

    let filtered_out = tests_len - filtered_tests.len();
    let event = TestEvent::TeFilteredOut(filtered_out);
    notify_about_test_event(event)?;

    let filtered_descs = filtered_tests.iter().map(|t| t.desc.clone()).collect();

    let event = TestEvent::TeFiltered(filtered_descs);
    notify_about_test_event(event)?;

    let (filtered_tests, filtered_benchs): (Vec<_>, _) = filtered_tests
        .into_iter()
        .partition(|e| matches!(e.testfn, StaticTestFn(_) | DynTestFn(_)));

    let concurrency = opts.test_threads.unwrap_or_else(get_concurrency);

    let mut remaining = filtered_tests;
    remaining.reverse();
    let mut pending = 0;

    let (tx, rx) = channel::<CompletedTest>();
    let run_strategy = if opts.options.panic_abort && !opts.force_run_in_process {
        RunStrategy::SpawnPrimary
    } else {
        RunStrategy::InProcess
    };

    let mut running_tests: TestMap = HashMap::default();
    let mut timeout_queue: VecDeque<TimeoutEntry> = VecDeque::new();

    fn get_timed_out_tests(
        running_tests: &TestMap,
        timeout_queue: &mut VecDeque<TimeoutEntry>,
    ) -> Vec<TestDesc> {
        let now = Instant::now();
        let mut timed_out = Vec::new();
        while let Some(timeout_entry) = timeout_queue.front() {
            if now < timeout_entry.timeout {
                break;
            }
            let timeout_entry = timeout_queue.pop_front().unwrap();
            if running_tests.contains_key(&timeout_entry.desc) {
                timed_out.push(timeout_entry.desc);
            }
        }
        timed_out
    }

    fn calc_timeout(timeout_queue: &VecDeque<TimeoutEntry>) -> Option<Duration> {
        timeout_queue.front().map(|&TimeoutEntry { timeout: next_timeout, .. }| {
            let now = Instant::now();
            if next_timeout >= now { next_timeout - now } else { Duration::new(0, 0) }
        })
    }

    if concurrency == 1 {
        while !remaining.is_empty() {
            let test = remaining.pop().unwrap();
            let event = TestEvent::TeWait(test.desc.clone());
            notify_about_test_event(event)?;
            run_test(opts, !opts.run_tests, test, run_strategy, tx.clone(), Concurrent::No)
                .unwrap_none();
            let completed_test = rx.recv().unwrap();

            let event = TestEvent::TeResult(completed_test);
            notify_about_test_event(event)?;
        }
    } else {
        while pending > 0 || !remaining.is_empty() {
            while pending < concurrency && !remaining.is_empty() {
                let test = remaining.pop().unwrap();
                let timeout = time::get_default_test_timeout();
                let desc = test.desc.clone();

                let event = TestEvent::TeWait(desc.clone());
                notify_about_test_event(event)?; //here no pad
                let join_handle = run_test(
                    opts,
                    !opts.run_tests,
                    test,
                    run_strategy,
                    tx.clone(),
                    Concurrent::Yes,
                );
                running_tests.insert(desc.clone(), RunningTest { join_handle });
                timeout_queue.push_back(TimeoutEntry { desc, timeout });
                pending += 1;
            }

            let mut res;
            loop {
                if let Some(timeout) = calc_timeout(&timeout_queue) {
                    res = rx.recv_timeout(timeout);
                    for test in get_timed_out_tests(&running_tests, &mut timeout_queue) {
                        let event = TestEvent::TeTimeout(test);
                        notify_about_test_event(event)?;
                    }

                    match res {
                        Err(RecvTimeoutError::Timeout) => {
                            // Result is not yet ready, continue waiting.
                        }
                        _ => {
                            // We've got a result, stop the loop.
                            break;
                        }
                    }
                } else {
                    res = rx.recv().map_err(|_| RecvTimeoutError::Disconnected);
                    break;
                }
            }

            let mut completed_test = res.unwrap();
            if let Some(running_test) = running_tests.remove(&completed_test.desc) {
                if let Some(join_handle) = running_test.join_handle {
                    if let Err(_) = join_handle.join() {
                        if let TrOk = completed_test.result {
                            completed_test.result =
                                TrFailedMsg("panicked after reporting success".to_string());
                        }
                    }
                }
            }

            let event = TestEvent::TeResult(completed_test);
            notify_about_test_event(event)?;
            pending -= 1;
        }
    }

    if opts.bench_benchmarks {
        // All benchmarks run at the end, in serial.
        for b in filtered_benchs {
            let event = TestEvent::TeWait(b.desc.clone());
            notify_about_test_event(event)?;
            run_test(opts, false, b, run_strategy, tx.clone(), Concurrent::No);
            let completed_test = rx.recv().unwrap();

            let event = TestEvent::TeResult(completed_test);
            notify_about_test_event(event)?;
        }
    }
    Ok(())
}

pub fn filter_tests(opts: &TestOpts, tests: Vec<TestDescAndFn>) -> Vec<TestDescAndFn> {
    let mut filtered = tests;
    let matches_filter = |test: &TestDescAndFn, filter: &str| {
        let test_name = test.desc.name.as_slice();

        match opts.filter_exact {
            true => test_name == filter,
            false => test_name.contains(filter),
        }
    };

    // Remove tests that don't match the test filter
    if let Some(ref filter) = opts.filter {
        filtered.retain(|test| matches_filter(test, filter));
    }

    // Skip tests that match any of the skip filters
    filtered.retain(|test| !opts.skip.iter().any(|sf| matches_filter(test, sf)));

    // Excludes #[should_panic] tests
    if opts.exclude_should_panic {
        filtered.retain(|test| test.desc.should_panic == ShouldPanic::No);
    }

    // maybe unignore tests
    match opts.run_ignored {
        RunIgnored::Yes => {
            filtered.iter_mut().for_each(|test| test.desc.ignore = false);
        }
        RunIgnored::Only => {
            filtered.retain(|test| test.desc.ignore);
            filtered.iter_mut().for_each(|test| test.desc.ignore = false);
        }
        RunIgnored::No => {}
    }

    // Sort the tests alphabetically
    filtered.sort_by(|t1, t2| t1.desc.name.as_slice().cmp(t2.desc.name.as_slice()));

    filtered
}

pub fn convert_benchmarks_to_tests(tests: Vec<TestDescAndFn>) -> Vec<TestDescAndFn> {
    // convert benchmarks to tests, if we're not benchmarking them
    tests
        .into_iter()
        .map(|x| {
            let testfn = match x.testfn {
                DynBenchFn(bench) => DynTestFn(Box::new(move || {
                    bench::run_once(|b| __rust_begin_short_backtrace(|| bench.run(b)))
                })),
                StaticBenchFn(benchfn) => DynTestFn(Box::new(move || {
                    bench::run_once(|b| __rust_begin_short_backtrace(|| benchfn(b)))
                })),
                f => f,
            };
            TestDescAndFn { desc: x.desc, testfn }
        })
        .collect()
}

pub fn run_test(
    opts: &TestOpts,
    force_ignore: bool,
    test: TestDescAndFn,
    strategy: RunStrategy,
    monitor_ch: Sender<CompletedTest>,
    concurrency: Concurrent,
) -> Option<thread::JoinHandle<()>> {
    let TestDescAndFn { desc, testfn } = test;

    // Emscripten can catch panics but other wasm targets cannot
    let ignore_because_no_process_support = desc.should_panic != ShouldPanic::No
        && cfg!(target_arch = "wasm32")
        && !cfg!(target_os = "emscripten");

    if force_ignore || desc.ignore || ignore_because_no_process_support {
        let message = CompletedTest::new(desc, TrIgnored, None, Vec::new());
        monitor_ch.send(message).unwrap();
        return None;
    }

    struct TestRunOpts {
        pub strategy: RunStrategy,
        pub nocapture: bool,
        pub concurrency: Concurrent,
        pub time: Option<time::TestTimeOptions>,
    }

    fn run_test_inner(
        desc: TestDesc,
        monitor_ch: Sender<CompletedTest>,
        testfn: Box<dyn FnOnce() + Send>,
        opts: TestRunOpts,
    ) -> Option<thread::JoinHandle<()>> {
        let concurrency = opts.concurrency;
        let name = desc.name.clone();

        let runtest = move || match opts.strategy {
            RunStrategy::InProcess => run_test_in_process(
                desc,
                opts.nocapture,
                opts.time.is_some(),
                testfn,
                monitor_ch,
                opts.time,
            ),
            RunStrategy::SpawnPrimary => spawn_test_subprocess(
                desc,
                opts.nocapture,
                opts.time.is_some(),
                monitor_ch,
                opts.time,
            ),
        };

        // If the platform is single-threaded we're just going to run
        // the test synchronously, regardless of the concurrency
        // level.
        let supports_threads = !cfg!(target_os = "emscripten") && !cfg!(target_arch = "wasm32");
        if concurrency == Concurrent::Yes && supports_threads {
            let cfg = thread::Builder::new().name(name.as_slice().to_owned());
            Some(cfg.spawn(runtest).unwrap())
        } else {
            runtest();
            None
        }
    }

    let test_run_opts =
        TestRunOpts { strategy, nocapture: opts.nocapture, concurrency, time: opts.time_options };

    match testfn {
        DynBenchFn(bencher) => {
            // Benchmarks aren't expected to panic, so we run them all in-process.
            crate::bench::benchmark(desc, monitor_ch, opts.nocapture, |harness| {
                bencher.run(harness)
            });
            None
        }
        StaticBenchFn(benchfn) => {
            // Benchmarks aren't expected to panic, so we run them all in-process.
            crate::bench::benchmark(desc, monitor_ch, opts.nocapture, benchfn);
            None
        }
        DynTestFn(f) => {
            match strategy {
                RunStrategy::InProcess => (),
                _ => panic!("Cannot run dynamic test fn out-of-process"),
            };
            run_test_inner(
                desc,
                monitor_ch,
                Box::new(move || __rust_begin_short_backtrace(f)),
                test_run_opts,
            )
        }
        StaticTestFn(f) => run_test_inner(
            desc,
            monitor_ch,
            Box::new(move || __rust_begin_short_backtrace(f)),
            test_run_opts,
        ),
    }
}

/// Fixed frame used to clean the backtrace with `RUST_BACKTRACE=1`.
#[inline(never)]
fn __rust_begin_short_backtrace<F: FnOnce()>(f: F) {
    f();

    // prevent this frame from being tail-call optimised away
    black_box(());
}

fn run_test_in_process(
    desc: TestDesc,
    nocapture: bool,
    report_time: bool,
    testfn: Box<dyn FnOnce() + Send>,
    monitor_ch: Sender<CompletedTest>,
    time_opts: Option<time::TestTimeOptions>,
) {
    // Buffer for capturing standard I/O
    let data = Arc::new(Mutex::new(Vec::new()));

    if !nocapture {
        io::set_output_capture(Some(data.clone()));
    }

    let start = report_time.then(Instant::now);
    let result = catch_unwind(AssertUnwindSafe(testfn));
    let exec_time = start.map(|start| {
        let duration = start.elapsed();
        TestExecTime(duration)
    });

    io::set_output_capture(None);

    let test_result = match result {
        Ok(()) => calc_result(&desc, Ok(()), &time_opts, &exec_time),
        Err(e) => calc_result(&desc, Err(e.as_ref()), &time_opts, &exec_time),
    };
    let stdout = data.lock().unwrap_or_else(|e| e.into_inner()).to_vec();
    let message = CompletedTest::new(desc, test_result, exec_time, stdout);
    monitor_ch.send(message).unwrap();
}

fn spawn_test_subprocess(
    desc: TestDesc,
    nocapture: bool,
    report_time: bool,
    monitor_ch: Sender<CompletedTest>,
    time_opts: Option<time::TestTimeOptions>,
) {
    let (result, test_output, exec_time) = (|| {
        let args = env::args().collect::<Vec<_>>();
        let current_exe = &args[0];

        let mut command = Command::new(current_exe);
        command.env(SECONDARY_TEST_INVOKER_VAR, desc.name.as_slice());
        if nocapture {
            command.stdout(process::Stdio::inherit());
            command.stderr(process::Stdio::inherit());
        }

        let start = report_time.then(Instant::now);
        let output = match command.output() {
            Ok(out) => out,
            Err(e) => {
                let err = format!("Failed to spawn {} as child for test: {:?}", args[0], e);
                return (TrFailed, err.into_bytes(), None);
            }
        };
        let exec_time = start.map(|start| {
            let duration = start.elapsed();
            TestExecTime(duration)
        });

        let std::process::Output { stdout, stderr, status } = output;
        let mut test_output = stdout;
        formatters::write_stderr_delimiter(&mut test_output, &desc.name);
        test_output.extend_from_slice(&stderr);

        let result = match (|| -> Result<TestResult, String> {
            let exit_code = get_exit_code(status)?;
            Ok(get_result_from_exit_code(&desc, exit_code, &time_opts, &exec_time))
        })() {
            Ok(r) => r,
            Err(e) => {
                write!(&mut test_output, "Unexpected error: {}", e).unwrap();
                TrFailed
            }
        };

        (result, test_output, exec_time)
    })();

    let message = CompletedTest::new(desc, result, exec_time, test_output);
    monitor_ch.send(message).unwrap();
}

fn run_test_in_spawned_subprocess(desc: TestDesc, testfn: Box<dyn FnOnce() + Send>) -> ! {
    let builtin_panic_hook = panic::take_hook();
    let record_result = Arc::new(move |panic_info: Option<&'_ PanicInfo<'_>>| {
        let test_result = match panic_info {
            Some(info) => calc_result(&desc, Err(info.payload()), &None, &None),
            None => calc_result(&desc, Ok(()), &None, &None),
        };

        // We don't support serializing TrFailedMsg, so just
        // print the message out to stderr.
        if let TrFailedMsg(msg) = &test_result {
            eprintln!("{}", msg);
        }

        if let Some(info) = panic_info {
            builtin_panic_hook(info);
        }

        if let TrOk = test_result {
            process::exit(test_result::TR_OK);
        } else {
            process::exit(test_result::TR_FAILED);
        }
    });
    let record_result2 = record_result.clone();
    panic::set_hook(Box::new(move |info| record_result2(Some(&info))));
    testfn();
    record_result(None);
    unreachable!("panic=abort callback should have exited the process")
}

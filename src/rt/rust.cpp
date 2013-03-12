// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/**
 * Main entry point into the Rust runtime. Here we initialize the kernel,
 * create the initial scheduler and run the main task.
 */

#include "rust_globals.h"
#include "rust_kernel.h"
#include "rust_util.h"
#include "rust_scheduler.h"
#include "rust_gc_metadata.h"

void* global_crate_map = NULL;

#ifndef _WIN32
pthread_key_t sched_key;
#else
DWORD sched_key;
#endif

extern "C" void*
rust_get_sched_tls_key() {
    return &sched_key;
}

/**
   The runtime entrypoint. The (C ABI) main function generated by rustc calls
   `rust_start`, providing the address of the Rust ABI main function, the
   platform argument vector, and a `crate_map` the provides some logging
   metadata.
*/
extern "C" CDECL int
rust_start(uintptr_t main_fn, int argc, char **argv, void* crate_map) {

#ifndef _WIN32
    pthread_key_create(&sched_key, NULL);
#endif

    // Load runtime configuration options from the environment.
    // FIXME #1497: Should provide a way to get these from the command
    // line as well.
    rust_env *env = load_env(argc, argv);

    global_crate_map = crate_map;

    update_gc_metadata(crate_map);

    update_log_settings(crate_map, env->logspec);

    rust_kernel *kernel = new rust_kernel(env);

    // Create the main task
    rust_sched_id sched_id = kernel->main_sched_id();
    rust_scheduler *sched = kernel->get_scheduler_by_id(sched_id);
    assert(sched != NULL);
    rust_task *root_task = sched->create_task(NULL, "main");

    // Schedule the main Rust task
    root_task->start((spawn_fn)main_fn, NULL, NULL);

    // At this point the task lifecycle is responsible for it
    // and our pointer may not be valid
    root_task = NULL;

    // Run the kernel until all schedulers exit
    int ret = kernel->run();

    delete kernel;
    free_env(env);

    return ret;
}

//
// Local Variables:
// mode: C++
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// End:
//

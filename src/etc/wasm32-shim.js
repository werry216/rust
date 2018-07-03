// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// This is a small "shim" program which is used when wasm32 unit tests are run
// in this repository. This program is intended to be run in node.js and will
// load a wasm module into memory, instantiate it with a set of imports, and
// then run it.
//
// There's a bunch of helper functions defined here in `imports.env`, but note
// that most of them aren't actually needed to execute most programs. Many of
// these are just intended for completeness or debugging. Hopefully over time
// nothing here is needed for completeness.

const fs = require('fs');
const process = require('process');
const buffer = fs.readFileSync(process.argv[2]);

Error.stackTraceLimit = 20;

let m = new WebAssembly.Module(buffer);

let memory = null;

function viewstruct(data, fields) {
  return new Uint32Array(memory.buffer).subarray(data/4, data/4 + fields);
}

function copystr(a, b) {
  let view = new Uint8Array(memory.buffer).subarray(a, a + b);
  return String.fromCharCode.apply(null, view);
}

function syscall_write([fd, ptr, len]) {
  let s = copystr(ptr, len);
  switch (fd) {
    case 1: process.stdout.write(s); break;
    case 2: process.stderr.write(s); break;
  }
}

function syscall_exit([code]) {
  process.exit(code);
}

function syscall_args(params) {
  let [ptr, len] = params;

  // Calculate total required buffer size
  let totalLen = -1;
  for (let i = 2; i < process.argv.length; ++i) {
    totalLen += Buffer.byteLength(process.argv[i]) + 1;
  }
  if (totalLen < 0) { totalLen = 0; }
  params[2] = totalLen;

  // If buffer is large enough, copy data
  if (len >= totalLen) {
    let view = new Uint8Array(memory.buffer);
    for (let i = 2; i < process.argv.length; ++i) {
      let value = process.argv[i];
      Buffer.from(value).copy(view, ptr);
      ptr += Buffer.byteLength(process.argv[i]) + 1;
    }
  }
}

function syscall_getenv(params) {
  let [keyPtr, keyLen, valuePtr, valueLen] = params;

  let key = copystr(keyPtr, keyLen);
  let value = process.env[key];

  if (value == null) {
    params[4] = 0xFFFFFFFF;
  } else {
    let view = new Uint8Array(memory.buffer);
    let totalLen = Buffer.byteLength(value);
    params[4] = totalLen;
    if (valueLen >= totalLen) {
      Buffer.from(value).copy(view, valuePtr);
    }
  }
}

function syscall_time(params) {
  let t = Date.now();
  let secs = Math.floor(t / 1000);
  let millis = t % 1000;
  params[1] = Math.floor(secs / 0x100000000);
  params[2] = secs % 0x100000000;
  params[3] = Math.floor(millis * 1000000);
}

let imports = {};
imports.env = {
  // These are generated by LLVM itself for various intrinsic calls. Hopefully
  // one day this is not necessary and something will automatically do this.
  fmod: function(x, y) { return x % y; },
  exp2: function(x) { return Math.pow(2, x); },
  exp2f: function(x) { return Math.pow(2, x); },
  ldexp: function(x, y) { return x * Math.pow(2, y); },
  ldexpf: function(x, y) { return x * Math.pow(2, y); },
  sin: Math.sin,
  sinf: Math.sin,
  cos: Math.cos,
  cosf: Math.cos,
  log: Math.log,
  log2: Math.log2,
  log10: Math.log10,
  log10f: Math.log10,

  rust_wasm_syscall: function(index, data) {
    switch (index) {
      case 1: syscall_write(viewstruct(data, 3)); return true;
      case 2: syscall_exit(viewstruct(data, 1)); return true;
      case 3: syscall_args(viewstruct(data, 3)); return true;
      case 4: syscall_getenv(viewstruct(data, 5)); return true;
      case 6: syscall_time(viewstruct(data, 4)); return true;
      default:
        console.log("Unsupported syscall: " + index);
        return false;
    }
  }
};

let instance = new WebAssembly.Instance(m, imports);
memory = instance.exports.memory;
try {
  instance.exports.main();
} catch (e) {
  console.error(e);
  process.exit(101);
}

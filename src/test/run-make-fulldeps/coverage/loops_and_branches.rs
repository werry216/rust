#![allow(unused_assignments)]

// This test confirms an earlier problem was resolved, supporting the MIR graph generated by the
// structure of this `fmt` function.

struct DebugTest;

impl std::fmt::Debug for DebugTest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if true {
            if false {
                while true {
                }
            }
            write!(f, "error")?;
        } else {
        }
        Ok(())
    }
}

fn main() {
    let debug_test = DebugTest;
    println!("{:?}", debug_test);
}

/*

This is the error message generated, before the issue was fixed:

error: internal compiler error: compiler/rustc_mir/src/transform/coverage/mod.rs:374:42:
Error processing: DefId(0:6 ~ bug_incomplete_cov_graph_traversal_simplified[317d]::{impl#0}::fmt):
Error { message: "`TraverseCoverageGraphWithLoops` missed some `BasicCoverageBlock`s:
[bcb6, bcb7, bcb9]" }

*/

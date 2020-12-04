// This test confirms an earlier problem was resolved, supporting the MIR graph generated by the
// structure of this test.

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: usize,
    minor: usize, // Count: 1 - `PartialOrd` compared `minor` values in 3.2.1 vs. 3.3.0
    patch: usize, // Count: 0 - `PartialOrd` was determined by `minor` (2 < 3)
}

impl Version {
    pub fn new(major: usize, minor: usize, patch: usize) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

fn main() {
    let version_3_2_1 = Version::new(3, 2, 1);
    let version_3_3_0 = Version::new(3, 3, 0);

    println!("{:?} < {:?} = {}", version_3_2_1, version_3_3_0, version_3_2_1 < version_3_3_0);
}

/*

This test verifies a bug was fixed that otherwise generated this error:

thread 'rustc' panicked at 'No counters provided the source_hash for function:
    Instance {
        def: Item(WithOptConstParam {
            did: DefId(0:101 ~ autocfg[c44a]::version::{impl#2}::partial_cmp),
            const_param_did: None
        }),
        substs: []
    }'
The `PartialOrd` derived by `Version` happened to generate a MIR that generated coverage
without a code region associated with any `Counter`. Code regions were associated with at least
one expression, which is allowed, but the `function_source_hash` was only passed to the codegen
(coverage mapgen) phase from a `Counter`s code region. A new method was added to pass the
`function_source_hash` without a code region, if necessary.

*/

// FIXME(#79626): The derived traits get coverage, which is great, but some of the traits appear
// to get two coverage execution counts at different positions:
//
// ```text
//    4|      2|#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
//                       ^0            ^0      ^0 ^0  ^1       ^0 ^0^0
// ```text
//
// `PartialEq`, `PartialOrd`, and `Ord` (and possibly `Eq`, if the trait name was longer than 2
// characters) have counts at their first and last characters.
//
// Why is this? Why does `PartialOrd` have two values (1 and 0)? This must mean we are checking
// distinct coverages, so maybe we don't want to eliminate one of them. Should we merge them?
// If merged, do we lose some information?

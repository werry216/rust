#![feature(lint_reasons)]

#![warn(absolute_paths_not_starting_with_crate, reason = 0)]
//~^ ERROR malformed lint attribute
//~| HELP reason must be a string literal
#![warn(anonymous_parameters, reason = b"consider these, for we have condemned them")]
//~^ ERROR malformed lint attribute
//~| HELP reason must be a string literal
#![warn(bare_trait_objects, reasons = "leaders to no sure land, guides their bearings lost")]
//~^ ERROR malformed lint attribute
#![warn(box_pointers, blerp = "or in league with robbers have reversed the signposts")]
//~^ ERROR malformed lint attribute
#![warn(elided_lifetimes_in_paths, reason("disrespectful to ancestors", "irresponsible to heirs"))]
//~^ ERROR malformed lint attribute
#![warn(ellipsis_inclusive_range_patterns, reason = "born barren", reason = "a freak growth")]
//~^ ERROR malformed lint attribute
//~| HELP reason in lint attribute must come last
#![warn(keyword_idents, reason = "root in rubble", macro_use_extern_crate)]
//~^ ERROR malformed lint attribute
//~| HELP reason in lint attribute must come last
#![warn(missing_copy_implementations, reason)]
//~^ WARN unknown lint

fn main() {}

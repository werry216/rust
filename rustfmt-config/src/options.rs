// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use config_type::ConfigType;
use lists::*;

/// Macro for deriving implementations of Serialize/Deserialize for enums
#[macro_export]
macro_rules! impl_enum_serialize_and_deserialize {
    ( $e:ident, $( $x:ident ),* ) => {
        impl ::serde::ser::Serialize for $e {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: ::serde::ser::Serializer
            {
                use serde::ser::Error;

                // We don't know whether the user of the macro has given us all options.
                #[allow(unreachable_patterns)]
                match *self {
                    $(
                        $e::$x => serializer.serialize_str(stringify!($x)),
                    )*
                    _ => {
                        Err(S::Error::custom(format!("Cannot serialize {:?}", self)))
                    }
                }
            }
        }

        impl<'de> ::serde::de::Deserialize<'de> for $e {
            fn deserialize<D>(d: D) -> Result<Self, D::Error>
                    where D: ::serde::Deserializer<'de> {
                use serde::de::{Error, Visitor};
                use std::marker::PhantomData;
                use std::fmt;
                struct StringOnly<T>(PhantomData<T>);
                impl<'de, T> Visitor<'de> for StringOnly<T>
                        where T: ::serde::Deserializer<'de> {
                    type Value = String;
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("string")
                    }
                    fn visit_str<E>(self, value: &str) -> Result<String, E> {
                        Ok(String::from(value))
                    }
                }
                let s = d.deserialize_string(StringOnly::<D>(PhantomData))?;
                $(
                    if stringify!($x).eq_ignore_ascii_case(&s) {
                      return Ok($e::$x);
                    }
                )*
                static ALLOWED: &'static[&str] = &[$(stringify!($x),)*];
                Err(D::Error::unknown_variant(&s, ALLOWED))
            }
        }

        impl ::std::str::FromStr for $e {
            type Err = &'static str;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                $(
                    if stringify!($x).eq_ignore_ascii_case(s) {
                        return Ok($e::$x);
                    }
                )*
                Err("Bad variant")
            }
        }

        impl ConfigType for $e {
            fn doc_hint() -> String {
                let mut variants = Vec::new();
                $(
                    variants.push(stringify!($x));
                )*
                format!("[{}]", variants.join("|"))
            }
        }
    };
}

macro_rules! configuration_option_enum{
    ($e:ident: $( $x:ident ),+ $(,)*) => {
        #[derive(Copy, Clone, Eq, PartialEq, Debug)]
        pub enum $e {
            $( $x ),+
        }

        impl_enum_serialize_and_deserialize!($e, $( $x ),+);
    }
}

configuration_option_enum! { NewlineStyle:
    Windows, // \r\n
    Unix, // \n
    Native, // \r\n in Windows, \n on other platforms
}

configuration_option_enum! { BraceStyle:
    AlwaysNextLine,
    PreferSameLine,
    // Prefer same line except where there is a where clause, in which case force
    // the brace to the next line.
    SameLineWhere,
}

configuration_option_enum! { ControlBraceStyle:
    // K&R style, Rust community default
    AlwaysSameLine,
    // Stroustrup style
    ClosingNextLine,
    // Allman style
    AlwaysNextLine,
}

configuration_option_enum! { IndentStyle:
    // First line on the same line as the opening brace, all lines aligned with
    // the first line.
    Visual,
    // First line is on a new line and all lines align with block indent.
    Block,
}

configuration_option_enum! { Density:
    // Fit as much on one line as possible.
    Compressed,
    // Use more lines.
    Tall,
    // Place every item on a separate line.
    Vertical,
}

configuration_option_enum! { TypeDensity:
    // No spaces around "=" and "+"
    Compressed,
    // Spaces around " = " and " + "
    Wide,
}

impl Density {
    pub fn to_list_tactic(self) -> ListTactic {
        match self {
            Density::Compressed => ListTactic::Mixed,
            Density::Tall => ListTactic::HorizontalVertical,
            Density::Vertical => ListTactic::Vertical,
        }
    }
}

configuration_option_enum! { ReportTactic:
    Always,
    Unnumbered,
    Never,
}

configuration_option_enum! { WriteMode:
    // Backs the original file up and overwrites the original.
    Replace,
    // Overwrites original file without backup.
    Overwrite,
    // Writes the output to stdout.
    Display,
    // Writes the diff to stdout.
    Diff,
    // Displays how much of the input file was processed
    Coverage,
    // Unfancy stdout
    Plain,
    // Outputs a checkstyle XML file.
    Checkstyle,
    // Output the changed lines (for internal value only)
    Modified,
}

configuration_option_enum! { Color:
    // Always use color, whether it is a piped or terminal output
    Always,
    // Never use color
    Never,
    // Automatically use color, if supported by terminal
    Auto,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct WidthHeuristics {
    // Maximum width of the args of a function call before falling back
    // to vertical formatting.
    pub fn_call_width: usize,
    // Maximum width in the body of a struct lit before falling back to
    // vertical formatting.
    pub struct_lit_width: usize,
    // Maximum width in the body of a struct variant before falling back
    // to vertical formatting.
    pub struct_variant_width: usize,
    // Maximum width of an array literal before falling back to vertical
    // formatting.
    pub array_width: usize,
    // Maximum length of a chain to fit on a single line.
    pub chain_width: usize,
    // Maximum line length for single line if-else expressions. A value
    // of zero means always break if-else expressions.
    pub single_line_if_else_max_width: usize,
}

impl WidthHeuristics {
    // Using this WidthHeuristics means we ignore heuristics.
    pub fn null() -> WidthHeuristics {
        WidthHeuristics {
            fn_call_width: usize::max_value(),
            struct_lit_width: 0,
            struct_variant_width: 0,
            array_width: usize::max_value(),
            chain_width: usize::max_value(),
            single_line_if_else_max_width: 0,
        }
    }
}

impl Default for WidthHeuristics {
    fn default() -> WidthHeuristics {
        WidthHeuristics {
            fn_call_width: 60,
            struct_lit_width: 18,
            struct_variant_width: 35,
            array_width: 60,
            chain_width: 60,
            single_line_if_else_max_width: 50,
        }
    }
}

impl ::std::str::FromStr for WidthHeuristics {
    type Err = &'static str;

    fn from_str(_: &str) -> Result<Self, Self::Err> {
        Err("WidthHeuristics is not parsable")
    }
}

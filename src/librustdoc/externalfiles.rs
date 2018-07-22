// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fs;
use std::path::Path;
use std::str;
use errors;
use syntax::feature_gate::UnstableFeatures;
use html::markdown::{ErrorCodes, Markdown};

#[derive(Clone)]
pub struct ExternalHtml {
    /// Content that will be included inline in the <head> section of a
    /// rendered Markdown file or generated documentation
    pub in_header: String,
    /// Content that will be included inline between <body> and the content of
    /// a rendered Markdown file or generated documentation
    pub before_content: String,
    /// Content that will be included inline between the content and </body> of
    /// a rendered Markdown file or generated documentation
    pub after_content: String
}

impl ExternalHtml {
    pub fn load(in_header: &[String], before_content: &[String], after_content: &[String],
                md_before_content: &[String], md_after_content: &[String], diag: &errors::Handler)
            -> Option<ExternalHtml> {
        let codes = ErrorCodes::from(UnstableFeatures::from_environment().is_nightly_build());
        load_external_files(in_header, diag)
            .and_then(|ih|
                load_external_files(before_content, diag)
                    .map(|bc| (ih, bc))
            )
            .and_then(|(ih, bc)|
                load_external_files(md_before_content, diag)
                    .map(|m_bc| (ih, format!("{}{}", bc, Markdown(&m_bc, &[], codes))))
            )
            .and_then(|(ih, bc)|
                load_external_files(after_content, diag)
                    .map(|ac| (ih, bc, ac))
            )
            .and_then(|(ih, bc, ac)|
                load_external_files(md_after_content, diag)
                    .map(|m_ac| (ih, bc, format!("{}{}", ac, Markdown(&m_ac, &[], codes))))
            )
            .map(|(ih, bc, ac)|
                ExternalHtml {
                    in_header: ih,
                    before_content: bc,
                    after_content: ac,
                }
            )
    }
}

pub enum LoadStringError {
    ReadFail,
    BadUtf8,
}

pub fn load_string<P: AsRef<Path>>(file_path: P, diag: &errors::Handler)
    -> Result<String, LoadStringError>
{
    let file_path = file_path.as_ref();
    let contents = match fs::read(file_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            diag.struct_err(&format!("error reading `{}`: {}", file_path.display(), e)).emit();
            return Err(LoadStringError::ReadFail);
        }
    };
    match str::from_utf8(&contents) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => {
            diag.struct_err(&format!("error reading `{}`: not UTF-8", file_path.display())).emit();
            Err(LoadStringError::BadUtf8)
        }
    }
}

fn load_external_files(names: &[String], diag: &errors::Handler) -> Option<String> {
    let mut out = String::new();
    for name in names {
        let s = match load_string(name, diag) {
            Ok(s) => s,
            Err(_) => return None,
        };
        out.push_str(&s);
        out.push('\n');
    }
    Some(out)
}

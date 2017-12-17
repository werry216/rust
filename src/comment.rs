// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Formatting and tools for comments.

use std::{self, iter};

use syntax::codemap::Span;

use config::Config;
use rewrite::RewriteContext;
use shape::{Indent, Shape};
use string::{rewrite_string, StringFormat};
use utils::{count_newlines, first_line_width, last_line_width};

fn is_custom_comment(comment: &str) -> bool {
    if !comment.starts_with("//") {
        false
    } else if let Some(c) = comment.chars().nth(2) {
        !c.is_alphanumeric() && !c.is_whitespace()
    } else {
        false
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum CommentStyle<'a> {
    DoubleSlash,
    TripleSlash,
    Doc,
    SingleBullet,
    DoubleBullet,
    Exclamation,
    Custom(&'a str),
}

fn custom_opener(s: &str) -> &str {
    s.lines().next().map_or("", |first_line| {
        first_line
            .find(' ')
            .map_or(first_line, |space_index| &first_line[0..space_index + 1])
    })
}

impl<'a> CommentStyle<'a> {
    pub fn opener(&self) -> &'a str {
        match *self {
            CommentStyle::DoubleSlash => "// ",
            CommentStyle::TripleSlash => "/// ",
            CommentStyle::Doc => "//! ",
            CommentStyle::SingleBullet => "/* ",
            CommentStyle::DoubleBullet => "/** ",
            CommentStyle::Exclamation => "/*! ",
            CommentStyle::Custom(opener) => opener,
        }
    }

    pub fn closer(&self) -> &'a str {
        match *self {
            CommentStyle::DoubleSlash
            | CommentStyle::TripleSlash
            | CommentStyle::Custom(..)
            | CommentStyle::Doc => "",
            CommentStyle::DoubleBullet => " **/",
            CommentStyle::SingleBullet | CommentStyle::Exclamation => " */",
        }
    }

    pub fn line_start(&self) -> &'a str {
        match *self {
            CommentStyle::DoubleSlash => "// ",
            CommentStyle::TripleSlash => "/// ",
            CommentStyle::Doc => "//! ",
            CommentStyle::SingleBullet | CommentStyle::Exclamation => " * ",
            CommentStyle::DoubleBullet => " ** ",
            CommentStyle::Custom(opener) => opener,
        }
    }

    pub fn to_str_tuplet(&self) -> (&'a str, &'a str, &'a str) {
        (self.opener(), self.closer(), self.line_start())
    }

    pub fn line_with_same_comment_style(&self, line: &str, normalize_comments: bool) -> bool {
        match *self {
            CommentStyle::DoubleSlash | CommentStyle::TripleSlash | CommentStyle::Doc => {
                line.trim_left().starts_with(self.line_start().trim_left())
                    || comment_style(line, normalize_comments) == *self
            }
            CommentStyle::DoubleBullet | CommentStyle::SingleBullet | CommentStyle::Exclamation => {
                line.trim_left().starts_with(self.closer().trim_left())
                    || line.trim_left().starts_with(self.line_start().trim_left())
                    || comment_style(line, normalize_comments) == *self
            }
            CommentStyle::Custom(opener) => line.trim_left().starts_with(opener.trim_right()),
        }
    }
}

fn comment_style(orig: &str, normalize_comments: bool) -> CommentStyle {
    if !normalize_comments {
        if orig.starts_with("/**") && !orig.starts_with("/**/") {
            CommentStyle::DoubleBullet
        } else if orig.starts_with("/*!") {
            CommentStyle::Exclamation
        } else if orig.starts_with("/*") {
            CommentStyle::SingleBullet
        } else if orig.starts_with("///") && orig.chars().nth(3).map_or(true, |c| c != '/') {
            CommentStyle::TripleSlash
        } else if orig.starts_with("//!") {
            CommentStyle::Doc
        } else if is_custom_comment(orig) {
            CommentStyle::Custom(custom_opener(orig))
        } else {
            CommentStyle::DoubleSlash
        }
    } else if (orig.starts_with("///") && orig.chars().nth(3).map_or(true, |c| c != '/'))
        || (orig.starts_with("/**") && !orig.starts_with("/**/"))
    {
        CommentStyle::TripleSlash
    } else if orig.starts_with("//!") || orig.starts_with("/*!") {
        CommentStyle::Doc
    } else if is_custom_comment(orig) {
        CommentStyle::Custom(custom_opener(orig))
    } else {
        CommentStyle::DoubleSlash
    }
}

pub fn combine_strs_with_missing_comments(
    context: &RewriteContext,
    prev_str: &str,
    next_str: &str,
    span: Span,
    shape: Shape,
    allow_extend: bool,
) -> Option<String> {
    let mut allow_one_line = !prev_str.contains('\n') && !next_str.contains('\n');
    let first_sep = if prev_str.is_empty() || next_str.is_empty() {
        ""
    } else {
        " "
    };
    let mut one_line_width =
        last_line_width(prev_str) + first_line_width(next_str) + first_sep.len();

    let indent_str = shape.indent.to_string(context.config);
    let missing_comment = rewrite_missing_comment(span, shape, context)?;

    if missing_comment.is_empty() {
        if allow_extend && prev_str.len() + first_sep.len() + next_str.len() <= shape.width {
            return Some(format!("{}{}{}", prev_str, first_sep, next_str));
        } else {
            let sep = if prev_str.is_empty() {
                String::new()
            } else {
                String::from("\n") + &indent_str
            };
            return Some(format!("{}{}{}", prev_str, sep, next_str));
        }
    }

    // We have a missing comment between the first expression and the second expression.

    // Peek the the original source code and find out whether there is a newline between the first
    // expression and the second expression or the missing comment. We will preserve the original
    // layout whenever possible.
    let original_snippet = context.snippet(span);
    let prefer_same_line = if let Some(pos) = original_snippet.chars().position(|c| c == '/') {
        !original_snippet[..pos].contains('\n')
    } else {
        !original_snippet.contains('\n')
    };

    one_line_width -= first_sep.len();
    let first_sep = if prev_str.is_empty() || missing_comment.is_empty() {
        String::new()
    } else {
        let one_line_width = last_line_width(prev_str) + first_line_width(&missing_comment) + 1;
        if prefer_same_line && one_line_width <= shape.width {
            String::from(" ")
        } else {
            format!("\n{}", indent_str)
        }
    };
    let second_sep = if missing_comment.is_empty() || next_str.is_empty() {
        String::new()
    } else if missing_comment.starts_with("//") {
        format!("\n{}", indent_str)
    } else {
        one_line_width += missing_comment.len() + first_sep.len() + 1;
        allow_one_line &= !missing_comment.starts_with("//") && !missing_comment.contains('\n');
        if prefer_same_line && allow_one_line && one_line_width <= shape.width {
            String::from(" ")
        } else {
            format!("\n{}", indent_str)
        }
    };
    Some(format!(
        "{}{}{}{}{}",
        prev_str, first_sep, missing_comment, second_sep, next_str,
    ))
}

pub fn rewrite_comment(
    orig: &str,
    block_style: bool,
    shape: Shape,
    config: &Config,
) -> Option<String> {
    // If there are lines without a starting sigil, we won't format them correctly
    // so in that case we won't even re-align (if !config.normalize_comments()) and
    // we should stop now.
    let num_bare_lines = orig.lines()
        .map(|line| line.trim())
        .filter(|l| !(l.starts_with('*') || l.starts_with("//") || l.starts_with("/*")))
        .count();
    if num_bare_lines > 0 && !config.normalize_comments() {
        return Some(orig.to_owned());
    }
    if !config.normalize_comments() && !config.wrap_comments() {
        return light_rewrite_comment(orig, shape.indent, config);
    }

    identify_comment(orig, block_style, shape, config)
}

fn identify_comment(
    orig: &str,
    block_style: bool,
    shape: Shape,
    config: &Config,
) -> Option<String> {
    let style = comment_style(orig, false);
    let first_group = orig.lines()
        .take_while(|l| style.line_with_same_comment_style(l, false))
        .collect::<Vec<_>>()
        .join("\n");
    let rest = orig.lines()
        .skip(first_group.lines().count())
        .collect::<Vec<_>>()
        .join("\n");

    let first_group_str = rewrite_comment_inner(&first_group, block_style, style, shape, config)?;
    if rest.is_empty() {
        Some(first_group_str)
    } else {
        identify_comment(&rest, block_style, shape, config).map(|rest_str| {
            format!(
                "{}\n{}{}",
                first_group_str,
                shape.indent.to_string(config),
                rest_str
            )
        })
    }
}

fn rewrite_comment_inner(
    orig: &str,
    block_style: bool,
    style: CommentStyle,
    shape: Shape,
    config: &Config,
) -> Option<String> {
    let (opener, closer, line_start) = if block_style {
        CommentStyle::SingleBullet.to_str_tuplet()
    } else {
        comment_style(orig, config.normalize_comments()).to_str_tuplet()
    };

    let max_chars = shape
        .width
        .checked_sub(closer.len() + opener.len())
        .unwrap_or(1);
    let indent_str = shape.indent.to_string(config);
    let fmt_indent = shape.indent + (opener.len() - line_start.len());
    let mut fmt = StringFormat {
        opener: "",
        closer: "",
        line_start: line_start,
        line_end: "",
        shape: Shape::legacy(max_chars, fmt_indent),
        trim_end: true,
        config: config,
    };

    let line_breaks = count_newlines(orig.trim_right());
    let lines = orig.lines()
        .enumerate()
        .map(|(i, mut line)| {
            line = line.trim();
            // Drop old closer.
            if i == line_breaks && line.ends_with("*/") && !line.starts_with("//") {
                line = line[..(line.len() - 2)].trim_right();
            }

            line
        })
        .map(|s| left_trim_comment_line(s, &style))
        .map(|(line, has_leading_whitespace)| {
            if orig.starts_with("/*") && line_breaks == 0 {
                (
                    line.trim_left(),
                    has_leading_whitespace || config.normalize_comments(),
                )
            } else {
                (line, has_leading_whitespace || config.normalize_comments())
            }
        });

    let mut result = String::with_capacity(orig.len() * 2);
    result.push_str(opener);
    let mut is_prev_line_multi_line = false;
    let mut inside_code_block = false;
    let comment_line_separator = format!("\n{}{}", indent_str, line_start);
    for (i, (line, has_leading_whitespace)) in lines.enumerate() {
        let is_last = i == count_newlines(orig);
        if result == opener {
            let force_leading_whitespace = opener == "/* " && count_newlines(orig) == 0;
            if !has_leading_whitespace && !force_leading_whitespace && result.ends_with(' ') {
                result.pop();
            }
            if line.is_empty() {
                continue;
            }
        } else if is_prev_line_multi_line && !line.is_empty() {
            result.push(' ')
        } else if is_last && !closer.is_empty() && line.is_empty() {
            result.push('\n');
            result.push_str(&indent_str);
        } else {
            result.push_str(&comment_line_separator);
            if !has_leading_whitespace && result.ends_with(' ') {
                result.pop();
            }
        }

        if line.starts_with("```") {
            inside_code_block = !inside_code_block;
        }
        if inside_code_block {
            if line.is_empty() && result.ends_with(' ') {
                result.pop();
            } else {
                result.push_str(line);
            }
            continue;
        }

        if config.wrap_comments() && line.len() > fmt.shape.width && !has_url(line) {
            match rewrite_string(line, &fmt, Some(max_chars)) {
                Some(ref s) => {
                    is_prev_line_multi_line = s.contains('\n');
                    result.push_str(s);
                }
                None if is_prev_line_multi_line => {
                    // We failed to put the current `line` next to the previous `line`.
                    // Remove the trailing space, then start rewrite on the next line.
                    result.pop();
                    result.push_str(&comment_line_separator);
                    fmt.shape = Shape::legacy(max_chars, fmt_indent);
                    match rewrite_string(line, &fmt, Some(max_chars)) {
                        Some(ref s) => {
                            is_prev_line_multi_line = s.contains('\n');
                            result.push_str(s);
                        }
                        None => {
                            is_prev_line_multi_line = false;
                            result.push_str(line);
                        }
                    }
                }
                None => {
                    is_prev_line_multi_line = false;
                    result.push_str(line);
                }
            }

            fmt.shape = if is_prev_line_multi_line {
                // 1 = " "
                let offset = 1 + last_line_width(&result) - line_start.len();
                Shape {
                    width: max_chars.checked_sub(offset).unwrap_or(0),
                    indent: fmt_indent,
                    offset: fmt.shape.offset + offset,
                }
            } else {
                Shape::legacy(max_chars, fmt_indent)
            };
        } else {
            if line.is_empty() && result.ends_with(' ') && !is_last {
                // Remove space if this is an empty comment or a doc comment.
                result.pop();
            }
            result.push_str(line);
            fmt.shape = Shape::legacy(max_chars, fmt_indent);
            is_prev_line_multi_line = false;
        }
    }

    result.push_str(closer);
    if result == opener && result.ends_with(' ') {
        // Trailing space.
        result.pop();
    }

    Some(result)
}

/// Returns true if the given string MAY include URLs or alike.
fn has_url(s: &str) -> bool {
    // This function may return false positive, but should get its job done in most cases.
    s.contains("https://") || s.contains("http://") || s.contains("ftp://") || s.contains("file://")
}

/// Given the span, rewrite the missing comment inside it if available.
/// Note that the given span must only include comments (or leading/trailing whitespaces).
pub fn rewrite_missing_comment(
    span: Span,
    shape: Shape,
    context: &RewriteContext,
) -> Option<String> {
    let missing_snippet = context.snippet(span);
    let trimmed_snippet = missing_snippet.trim();
    if !trimmed_snippet.is_empty() {
        rewrite_comment(trimmed_snippet, false, shape, context.config)
    } else {
        Some(String::new())
    }
}

/// Recover the missing comments in the specified span, if available.
/// The layout of the comments will be preserved as long as it does not break the code
/// and its total width does not exceed the max width.
pub fn recover_missing_comment_in_span(
    span: Span,
    shape: Shape,
    context: &RewriteContext,
    used_width: usize,
) -> Option<String> {
    let missing_comment = rewrite_missing_comment(span, shape, context)?;
    if missing_comment.is_empty() {
        Some(String::new())
    } else {
        let missing_snippet = context.snippet(span);
        let pos = missing_snippet.chars().position(|c| c == '/').unwrap_or(0);
        // 1 = ` `
        let total_width = missing_comment.len() + used_width + 1;
        let force_new_line_before_comment =
            missing_snippet[..pos].contains('\n') || total_width > context.config.max_width();
        let sep = if force_new_line_before_comment {
            format!("\n{}", shape.indent.to_string(context.config))
        } else {
            String::from(" ")
        };
        Some(format!("{}{}", sep, missing_comment))
    }
}

/// Trims whitespace and aligns to indent, but otherwise does not change comments.
fn light_rewrite_comment(orig: &str, offset: Indent, config: &Config) -> Option<String> {
    let lines: Vec<&str> = orig.lines()
        .map(|l| {
            // This is basically just l.trim(), but in the case that a line starts
            // with `*` we want to leave one space before it, so it aligns with the
            // `*` in `/*`.
            let first_non_whitespace = l.find(|c| !char::is_whitespace(c));
            if let Some(fnw) = first_non_whitespace {
                if l.as_bytes()[fnw] == b'*' && fnw > 0 {
                    &l[fnw - 1..]
                } else {
                    &l[fnw..]
                }
            } else {
                ""
            }.trim_right()
        })
        .collect();
    Some(lines.join(&format!("\n{}", offset.to_string(config))))
}

/// Trims comment characters and possibly a single space from the left of a string.
/// Does not trim all whitespace. If a single space is trimmed from the left of the string,
/// this function returns true.
fn left_trim_comment_line<'a>(line: &'a str, style: &CommentStyle) -> (&'a str, bool) {
    if line.starts_with("//! ") || line.starts_with("/// ") || line.starts_with("/*! ")
        || line.starts_with("/** ")
    {
        (&line[4..], true)
    } else if let CommentStyle::Custom(opener) = *style {
        if line.starts_with(opener) {
            (&line[opener.len()..], true)
        } else {
            (&line[opener.trim_right().len()..], false)
        }
    } else if line.starts_with("/* ") || line.starts_with("// ") || line.starts_with("//!")
        || line.starts_with("///") || line.starts_with("** ")
        || line.starts_with("/*!")
        || (line.starts_with("/**") && !line.starts_with("/**/"))
    {
        (&line[3..], line.chars().nth(2).unwrap() == ' ')
    } else if line.starts_with("/*") || line.starts_with("* ") || line.starts_with("//")
        || line.starts_with("**")
    {
        (&line[2..], line.chars().nth(1).unwrap() == ' ')
    } else if line.starts_with('*') {
        (&line[1..], false)
    } else {
        (line, line.starts_with(' '))
    }
}

pub trait FindUncommented {
    fn find_uncommented(&self, pat: &str) -> Option<usize>;
}

impl FindUncommented for str {
    fn find_uncommented(&self, pat: &str) -> Option<usize> {
        let mut needle_iter = pat.chars();
        for (kind, (i, b)) in CharClasses::new(self.char_indices()) {
            match needle_iter.next() {
                None => {
                    return Some(i - pat.len());
                }
                Some(c) => match kind {
                    FullCodeCharKind::Normal | FullCodeCharKind::InString if b == c => {}
                    _ => {
                        needle_iter = pat.chars();
                    }
                },
            }
        }

        // Handle case where the pattern is a suffix of the search string
        match needle_iter.next() {
            Some(_) => None,
            None => Some(self.len() - pat.len()),
        }
    }
}

// Returns the first byte position after the first comment. The given string
// is expected to be prefixed by a comment, including delimiters.
// Good: "/* /* inner */ outer */ code();"
// Bad:  "code(); // hello\n world!"
pub fn find_comment_end(s: &str) -> Option<usize> {
    let mut iter = CharClasses::new(s.char_indices());
    for (kind, (i, _c)) in &mut iter {
        if kind == FullCodeCharKind::Normal || kind == FullCodeCharKind::InString {
            return Some(i);
        }
    }

    // Handle case where the comment ends at the end of s.
    if iter.status == CharClassesStatus::Normal {
        Some(s.len())
    } else {
        None
    }
}

/// Returns true if text contains any comment.
pub fn contains_comment(text: &str) -> bool {
    CharClasses::new(text.chars()).any(|(kind, _)| kind.is_comment())
}

/// Remove trailing spaces from the specified snippet. We do not remove spaces
/// inside strings or comments.
pub fn remove_trailing_white_spaces(text: &str) -> String {
    let mut buffer = String::with_capacity(text.len());
    let mut space_buffer = String::with_capacity(128);
    for (char_kind, c) in CharClasses::new(text.chars()) {
        match c {
            '\n' => {
                if char_kind == FullCodeCharKind::InString {
                    buffer.push_str(&space_buffer);
                }
                space_buffer.clear();
                buffer.push('\n');
            }
            _ if c.is_whitespace() => {
                space_buffer.push(c);
            }
            _ => {
                if !space_buffer.is_empty() {
                    buffer.push_str(&space_buffer);
                    space_buffer.clear();
                }
                buffer.push(c);
            }
        }
    }
    buffer
}

pub struct CharClasses<T>
where
    T: Iterator,
    T::Item: RichChar,
{
    base: iter::Peekable<T>,
    status: CharClassesStatus,
}

pub trait RichChar {
    fn get_char(&self) -> char;
}

impl RichChar for char {
    fn get_char(&self) -> char {
        *self
    }
}

impl RichChar for (usize, char) {
    fn get_char(&self) -> char {
        self.1
    }
}

impl RichChar for (char, usize) {
    fn get_char(&self) -> char {
        self.0
    }
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum CharClassesStatus {
    Normal,
    LitString,
    LitStringEscape,
    LitChar,
    LitCharEscape,
    // The u32 is the nesting deepness of the comment
    BlockComment(u32),
    // Status when the '/' has been consumed, but not yet the '*', deepness is
    // the new deepness (after the comment opening).
    BlockCommentOpening(u32),
    // Status when the '*' has been consumed, but not yet the '/', deepness is
    // the new deepness (after the comment closing).
    BlockCommentClosing(u32),
    LineComment,
}

/// Distinguish between functional part of code and comments
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum CodeCharKind {
    Normal,
    Comment,
}

/// Distinguish between functional part of code and comments,
/// describing opening and closing of comments for ease when chunking
/// code from tagged characters
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum FullCodeCharKind {
    Normal,
    /// The first character of a comment, there is only one for a comment (always '/')
    StartComment,
    /// Any character inside a comment including the second character of comment
    /// marks ("//", "/*")
    InComment,
    /// Last character of a comment, '\n' for a line comment, '/' for a block comment.
    EndComment,
    /// Inside a string.
    InString,
}

impl FullCodeCharKind {
    pub fn is_comment(&self) -> bool {
        match *self {
            FullCodeCharKind::StartComment
            | FullCodeCharKind::InComment
            | FullCodeCharKind::EndComment => true,
            _ => false,
        }
    }

    pub fn is_string(&self) -> bool {
        *self == FullCodeCharKind::InString
    }

    fn to_codecharkind(&self) -> CodeCharKind {
        if self.is_comment() {
            CodeCharKind::Comment
        } else {
            CodeCharKind::Normal
        }
    }
}

impl<T> CharClasses<T>
where
    T: Iterator,
    T::Item: RichChar,
{
    pub fn new(base: T) -> CharClasses<T> {
        CharClasses {
            base: base.peekable(),
            status: CharClassesStatus::Normal,
        }
    }
}

impl<T> Iterator for CharClasses<T>
where
    T: Iterator,
    T::Item: RichChar,
{
    type Item = (FullCodeCharKind, T::Item);

    fn next(&mut self) -> Option<(FullCodeCharKind, T::Item)> {
        let item = self.base.next()?;
        let chr = item.get_char();
        let mut char_kind = FullCodeCharKind::Normal;
        self.status = match self.status {
            CharClassesStatus::LitString => match chr {
                '"' => CharClassesStatus::Normal,
                '\\' => {
                    char_kind = FullCodeCharKind::InString;
                    CharClassesStatus::LitStringEscape
                }
                _ => {
                    char_kind = FullCodeCharKind::InString;
                    CharClassesStatus::LitString
                }
            },
            CharClassesStatus::LitStringEscape => {
                char_kind = FullCodeCharKind::InString;
                CharClassesStatus::LitString
            }
            CharClassesStatus::LitChar => match chr {
                '\\' => CharClassesStatus::LitCharEscape,
                '\'' => CharClassesStatus::Normal,
                _ => CharClassesStatus::LitChar,
            },
            CharClassesStatus::LitCharEscape => CharClassesStatus::LitChar,
            CharClassesStatus::Normal => match chr {
                '"' => {
                    char_kind = FullCodeCharKind::InString;
                    CharClassesStatus::LitString
                }
                '\'' => CharClassesStatus::LitChar,
                '/' => match self.base.peek() {
                    Some(next) if next.get_char() == '*' => {
                        self.status = CharClassesStatus::BlockCommentOpening(1);
                        return Some((FullCodeCharKind::StartComment, item));
                    }
                    Some(next) if next.get_char() == '/' => {
                        self.status = CharClassesStatus::LineComment;
                        return Some((FullCodeCharKind::StartComment, item));
                    }
                    _ => CharClassesStatus::Normal,
                },
                _ => CharClassesStatus::Normal,
            },
            CharClassesStatus::BlockComment(deepness) => {
                assert_ne!(deepness, 0);
                self.status = match self.base.peek() {
                    Some(next) if next.get_char() == '/' && chr == '*' => {
                        CharClassesStatus::BlockCommentClosing(deepness - 1)
                    }
                    Some(next) if next.get_char() == '*' && chr == '/' => {
                        CharClassesStatus::BlockCommentOpening(deepness + 1)
                    }
                    _ => CharClassesStatus::BlockComment(deepness),
                };
                return Some((FullCodeCharKind::InComment, item));
            }
            CharClassesStatus::BlockCommentOpening(deepness) => {
                assert_eq!(chr, '*');
                self.status = CharClassesStatus::BlockComment(deepness);
                return Some((FullCodeCharKind::InComment, item));
            }
            CharClassesStatus::BlockCommentClosing(deepness) => {
                assert_eq!(chr, '/');
                if deepness == 0 {
                    self.status = CharClassesStatus::Normal;
                    return Some((FullCodeCharKind::EndComment, item));
                } else {
                    self.status = CharClassesStatus::BlockComment(deepness);
                    return Some((FullCodeCharKind::InComment, item));
                }
            }
            CharClassesStatus::LineComment => match chr {
                '\n' => {
                    self.status = CharClassesStatus::Normal;
                    return Some((FullCodeCharKind::EndComment, item));
                }
                _ => {
                    self.status = CharClassesStatus::LineComment;
                    return Some((FullCodeCharKind::InComment, item));
                }
            },
        };
        Some((char_kind, item))
    }
}

/// Iterator over functional and commented parts of a string. Any part of a string is either
/// functional code, either *one* block comment, either *one* line comment. Whitespace between
/// comments is functional code. Line comments contain their ending newlines.
struct UngroupedCommentCodeSlices<'a> {
    slice: &'a str,
    iter: iter::Peekable<CharClasses<std::str::CharIndices<'a>>>,
}

impl<'a> UngroupedCommentCodeSlices<'a> {
    fn new(code: &'a str) -> UngroupedCommentCodeSlices<'a> {
        UngroupedCommentCodeSlices {
            slice: code,
            iter: CharClasses::new(code.char_indices()).peekable(),
        }
    }
}

impl<'a> Iterator for UngroupedCommentCodeSlices<'a> {
    type Item = (CodeCharKind, usize, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let (kind, (start_idx, _)) = self.iter.next()?;
        match kind {
            FullCodeCharKind::Normal | FullCodeCharKind::InString => {
                // Consume all the Normal code
                while let Some(&(char_kind, _)) = self.iter.peek() {
                    if char_kind.is_comment() {
                        break;
                    }
                    let _ = self.iter.next();
                }
            }
            FullCodeCharKind::StartComment => {
                // Consume the whole comment
                while let Some((FullCodeCharKind::InComment, (_, _))) = self.iter.next() {}
            }
            _ => panic!(),
        }
        let slice = match self.iter.peek() {
            Some(&(_, (end_idx, _))) => &self.slice[start_idx..end_idx],
            None => &self.slice[start_idx..],
        };
        Some((
            if kind.is_comment() {
                CodeCharKind::Comment
            } else {
                CodeCharKind::Normal
            },
            start_idx,
            slice,
        ))
    }
}

/// Iterator over an alternating sequence of functional and commented parts of
/// a string. The first item is always a, possibly zero length, subslice of
/// functional text. Line style comments contain their ending newlines.
pub struct CommentCodeSlices<'a> {
    slice: &'a str,
    last_slice_kind: CodeCharKind,
    last_slice_end: usize,
}

impl<'a> CommentCodeSlices<'a> {
    pub fn new(slice: &'a str) -> CommentCodeSlices<'a> {
        CommentCodeSlices {
            slice: slice,
            last_slice_kind: CodeCharKind::Comment,
            last_slice_end: 0,
        }
    }
}

impl<'a> Iterator for CommentCodeSlices<'a> {
    type Item = (CodeCharKind, usize, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if self.last_slice_end == self.slice.len() {
            return None;
        }

        let mut sub_slice_end = self.last_slice_end;
        let mut first_whitespace = None;
        let subslice = &self.slice[self.last_slice_end..];
        let mut iter = CharClasses::new(subslice.char_indices());

        for (kind, (i, c)) in &mut iter {
            let is_comment_connector = self.last_slice_kind == CodeCharKind::Normal
                && &subslice[..2] == "//"
                && [' ', '\t'].contains(&c);

            if is_comment_connector && first_whitespace.is_none() {
                first_whitespace = Some(i);
            }

            if kind.to_codecharkind() == self.last_slice_kind && !is_comment_connector {
                let last_index = match first_whitespace {
                    Some(j) => j,
                    None => i,
                };
                sub_slice_end = self.last_slice_end + last_index;
                break;
            }

            if !is_comment_connector {
                first_whitespace = None;
            }
        }

        if let (None, true) = (iter.next(), sub_slice_end == self.last_slice_end) {
            // This was the last subslice.
            sub_slice_end = match first_whitespace {
                Some(i) => self.last_slice_end + i,
                None => self.slice.len(),
            };
        }

        let kind = match self.last_slice_kind {
            CodeCharKind::Comment => CodeCharKind::Normal,
            CodeCharKind::Normal => CodeCharKind::Comment,
        };
        let res = (
            kind,
            self.last_slice_end,
            &self.slice[self.last_slice_end..sub_slice_end],
        );
        self.last_slice_end = sub_slice_end;
        self.last_slice_kind = kind;

        Some(res)
    }
}

/// Checks is `new` didn't miss any comment from `span`, if it removed any, return previous text
/// (if it fits in the width/offset, else return None), else return `new`
pub fn recover_comment_removed(
    new: String,
    span: Span,
    context: &RewriteContext,
) -> Option<String> {
    let snippet = context.snippet(span);
    if snippet != new && changed_comment_content(snippet, &new) {
        // We missed some comments. Keep the original text.
        Some(snippet.to_owned())
    } else {
        Some(new)
    }
}

/// Return true if the two strings of code have the same payload of comments.
/// The payload of comments is everything in the string except:
///     - actual code (not comments)
///     - comment start/end marks
///     - whitespace
///     - '*' at the beginning of lines in block comments
fn changed_comment_content(orig: &str, new: &str) -> bool {
    // Cannot write this as a fn since we cannot return types containing closures
    let code_comment_content = |code| {
        let slices = UngroupedCommentCodeSlices::new(code);
        slices
            .filter(|&(ref kind, _, _)| *kind == CodeCharKind::Comment)
            .flat_map(|(_, _, s)| CommentReducer::new(s))
    };
    let res = code_comment_content(orig).ne(code_comment_content(new));
    debug!(
        "comment::changed_comment_content: {}\norig: '{}'\nnew: '{}'\nraw_old: {}\nraw_new: {}",
        res,
        orig,
        new,
        code_comment_content(orig).collect::<String>(),
        code_comment_content(new).collect::<String>()
    );
    res
}

/// Iterator over the 'payload' characters of a comment.
/// It skips whitespace, comment start/end marks, and '*' at the beginning of lines.
/// The comment must be one comment, ie not more than one start mark (no multiple line comments,
/// for example).
struct CommentReducer<'a> {
    is_block: bool,
    at_start_line: bool,
    iter: std::str::Chars<'a>,
}

impl<'a> CommentReducer<'a> {
    fn new(comment: &'a str) -> CommentReducer<'a> {
        let is_block = comment.starts_with("/*");
        let comment = remove_comment_header(comment);
        CommentReducer {
            is_block: is_block,
            at_start_line: false, // There are no supplementary '*' on the first line
            iter: comment.chars(),
        }
    }
}

impl<'a> Iterator for CommentReducer<'a> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut c = self.iter.next()?;
            if self.is_block && self.at_start_line {
                while c.is_whitespace() {
                    c = self.iter.next()?;
                }
                // Ignore leading '*'
                if c == '*' {
                    c = self.iter.next()?;
                }
            } else if c == '\n' {
                self.at_start_line = true;
            }
            if !c.is_whitespace() {
                return Some(c);
            }
        }
    }
}

fn remove_comment_header(comment: &str) -> &str {
    if comment.starts_with("///") || comment.starts_with("//!") {
        &comment[3..]
    } else if comment.starts_with("//") {
        &comment[2..]
    } else if (comment.starts_with("/**") && !comment.starts_with("/**/"))
        || comment.starts_with("/*!")
    {
        &comment[3..comment.len() - 2]
    } else {
        assert!(
            comment.starts_with("/*"),
            format!("string '{}' is not a comment", comment)
        );
        &comment[2..comment.len() - 2]
    }
}

#[cfg(test)]
mod test {
    use super::{contains_comment, rewrite_comment, CharClasses, CodeCharKind, CommentCodeSlices,
                FindUncommented, FullCodeCharKind};
    use shape::{Indent, Shape};

    #[test]
    fn char_classes() {
        let mut iter = CharClasses::new("//\n\n".chars());

        assert_eq!((FullCodeCharKind::StartComment, '/'), iter.next().unwrap());
        assert_eq!((FullCodeCharKind::InComment, '/'), iter.next().unwrap());
        assert_eq!((FullCodeCharKind::EndComment, '\n'), iter.next().unwrap());
        assert_eq!((FullCodeCharKind::Normal, '\n'), iter.next().unwrap());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn comment_code_slices() {
        let input = "code(); /* test */ 1 + 1";
        let mut iter = CommentCodeSlices::new(input);

        assert_eq!((CodeCharKind::Normal, 0, "code(); "), iter.next().unwrap());
        assert_eq!(
            (CodeCharKind::Comment, 8, "/* test */"),
            iter.next().unwrap()
        );
        assert_eq!((CodeCharKind::Normal, 18, " 1 + 1"), iter.next().unwrap());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn comment_code_slices_two() {
        let input = "// comment\n    test();";
        let mut iter = CommentCodeSlices::new(input);

        assert_eq!((CodeCharKind::Normal, 0, ""), iter.next().unwrap());
        assert_eq!(
            (CodeCharKind::Comment, 0, "// comment\n"),
            iter.next().unwrap()
        );
        assert_eq!(
            (CodeCharKind::Normal, 11, "    test();"),
            iter.next().unwrap()
        );
        assert_eq!(None, iter.next());
    }

    #[test]
    fn comment_code_slices_three() {
        let input = "1 // comment\n    // comment2\n\n";
        let mut iter = CommentCodeSlices::new(input);

        assert_eq!((CodeCharKind::Normal, 0, "1 "), iter.next().unwrap());
        assert_eq!(
            (CodeCharKind::Comment, 2, "// comment\n    // comment2\n"),
            iter.next().unwrap()
        );
        assert_eq!((CodeCharKind::Normal, 29, "\n"), iter.next().unwrap());
        assert_eq!(None, iter.next());
    }

    #[test]
    #[cfg_attr(rustfmt, rustfmt_skip)]
    fn format_comments() {
        let mut config: ::config::Config = Default::default();
        config.set().wrap_comments(true);
        config.set().normalize_comments(true);

        let comment = rewrite_comment(" //test",
                                      true,
                                      Shape::legacy(100, Indent::new(0, 100)),
                                      &config).unwrap();
        assert_eq!("/* test */", comment);

        let comment = rewrite_comment("// comment on a",
                                      false,
                                      Shape::legacy(10, Indent::empty()),
                                      &config).unwrap();
        assert_eq!("// comment\n// on a", comment);

        let comment = rewrite_comment("//  A multi line comment\n             // between args.",
                                      false,
                                      Shape::legacy(60, Indent::new(0, 12)),
                                      &config).unwrap();
        assert_eq!("//  A multi line comment\n            // between args.", comment);

        let input = "// comment";
        let expected =
            "/* comment */";
        let comment = rewrite_comment(input,
                                      true,
                                      Shape::legacy(9, Indent::new(0, 69)),
                                      &config).unwrap();
        assert_eq!(expected, comment);

        let comment = rewrite_comment("/*   trimmed    */",
                                      true,
                                      Shape::legacy(100, Indent::new(0, 100)),
                                      &config).unwrap();
        assert_eq!("/* trimmed */", comment);
    }

    // This is probably intended to be a non-test fn, but it is not used. I'm
    // keeping it around unless it helps us test stuff.
    fn uncommented(text: &str) -> String {
        CharClasses::new(text.chars())
            .filter_map(|(s, c)| match s {
                FullCodeCharKind::Normal | FullCodeCharKind::InString => Some(c),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_uncommented() {
        assert_eq!(&uncommented("abc/*...*/"), "abc");
        assert_eq!(
            &uncommented("// .... /* \n../* /* *** / */ */a/* // */c\n"),
            "..ac\n"
        );
        assert_eq!(&uncommented("abc \" /* */\" qsdf"), "abc \" /* */\" qsdf");
    }

    #[test]
    fn test_contains_comment() {
        assert_eq!(contains_comment("abc"), false);
        assert_eq!(contains_comment("abc // qsdf"), true);
        assert_eq!(contains_comment("abc /* kqsdf"), true);
        assert_eq!(contains_comment("abc \" /* */\" qsdf"), false);
    }

    #[test]
    fn test_find_uncommented() {
        fn check(haystack: &str, needle: &str, expected: Option<usize>) {
            assert_eq!(expected, haystack.find_uncommented(needle));
        }

        check("/*/ */test", "test", Some(6));
        check("//test\ntest", "test", Some(7));
        check("/* comment only */", "whatever", None);
        check(
            "/* comment */ some text /* more commentary */ result",
            "result",
            Some(46),
        );
        check("sup // sup", "p", Some(2));
        check("sup", "x", None);
        check(r#"π? /**/ π is nice!"#, r#"π is nice"#, Some(9));
        check("/*sup yo? \n sup*/ sup", "p", Some(20));
        check("hel/*lohello*/lo", "hello", None);
        check("acb", "ab", None);
        check(",/*A*/ ", ",", Some(0));
        check("abc", "abc", Some(0));
        check("/* abc */", "abc", None);
        check("/**/abc/* */", "abc", Some(4));
        check("\"/* abc */\"", "abc", Some(4));
        check("\"/* abc", "abc", Some(4));
    }
}

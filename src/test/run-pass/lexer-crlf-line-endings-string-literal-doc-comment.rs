// ignore-tidy-cr ignore-license
// ignore-tidy-cr (repeated again because of tidy bug)
// license is ignored because tidy can't handle the CRLF here properly.

// http://rust-lang.org/COPYRIGHT.
//

// N.B., this file needs CRLF line endings. The .gitattributes file in
// this directory should enforce it.

// ignore-pretty issue #37195

/// Doc comment that ends in CRLF
pub fn foo() {}

/** Block doc comment that
 *  contains CRLF characters
 */
pub fn bar() {}

fn main() {
    let s = "string
literal";
    assert_eq!(s, "string\nliteral");

    let s = "literal with \
             escaped newline";
    assert_eq!(s, "literal with escaped newline");

    let s = r"string
literal";
    assert_eq!(s, "string\nliteral");

    // validate that our source file has CRLF endings
    let source = include_str!("lexer-crlf-line-endings-string-literal-doc-comment.rs");
    assert!(source.contains("string\r\nliteral"));
}

//! This file tests for the DOC_MARKDOWN lint
//~^ ERROR: you should put `DOC_MARKDOWN` between ticks

#![feature(plugin)]
#![plugin(clippy)]

#![deny(doc_markdown)]

/// The foo_bar function does _nothing_. See also foo::bar. (note the dot there)
/// Markdown is _weird_. I mean _really weird_.  This \_ is ok. So is `_`. But not Foo::some_fun
/// which should be reported only once despite being __doubly bad__.
/// be_sure_we_got_to_the_end_of_it
fn foo_bar() {
//~^ ERROR: you should put `foo_bar` between ticks
//~| ERROR: you should put `foo::bar` between ticks
//~| ERROR: you should put `Foo::some_fun` between ticks
//~| ERROR: you should put `be_sure_we_got_to_the_end_of_it` between ticks
}

/// That one tests multiline ticks.
/// ```rust
/// foo_bar FOO_BAR
/// _foo bar_
/// ```
/// be_sure_we_got_to_the_end_of_it
fn multiline_ticks() {
//~^ ERROR: you should put `be_sure_we_got_to_the_end_of_it` between ticks
}

/// This _is a test for
/// multiline
/// emphasis_.
/// be_sure_we_got_to_the_end_of_it
fn test_emphasis() {
//~^ ERROR: you should put `be_sure_we_got_to_the_end_of_it` between ticks
}

/// This tests units. See also #835.
/// kiB MiB GiB TiB PiB EiB
/// kib Mib Gib Tib Pib Eib
/// kB MB GB TB PB EB
/// kb Mb Gb Tb Pb Eb
/// 32kiB 32MiB 32GiB 32TiB 32PiB 32EiB
/// 32kib 32Mib 32Gib 32Tib 32Pib 32Eib
/// 32kB 32MB 32GB 32TB 32PB 32EB
/// 32kb 32Mb 32Gb 32Tb 32Pb 32Eb
/// be_sure_we_got_to_the_end_of_it
fn test_units() {
//~^ ERROR: you should put `be_sure_we_got_to_the_end_of_it` between ticks
}

/// This one checks we don’t try to split unicode codepoints
/// `ß`
/// `ℝ`
/// `💣`
/// `❤️`
/// ß_foo
/// ℝ_foo
/// 💣_foo
/// ❤️_foo
/// foo_ß
/// foo_ℝ
/// foo_💣
/// foo_❤️
/// [ßdummy textß][foo_ß]
/// [ℝdummy textℝ][foo_ℝ]
/// [💣dummy tex💣t][foo_💣]
/// [❤️dummy text❤️][foo_❤️]
/// [ßdummy textß](foo_ß)
/// [ℝdummy textℝ](foo_ℝ)
/// [💣dummy tex💣t](foo_💣)
/// [❤️dummy text❤️](foo_❤️)
/// [foo_ß]: dummy text
/// [foo_ℝ]: dummy text
/// [foo_💣]: dummy text
/// [foo_❤️]: dummy text
/// be_sure_we_got_to_the_end_of_it
fn test_unicode() {
//~^ ERROR: you should put `ß_foo` between ticks
//~| ERROR: you should put `ℝ_foo` between ticks
//~| ERROR: you should put `foo_ß` between ticks
//~| ERROR: you should put `foo_ℝ` between ticks
//~| ERROR: you should put `be_sure_we_got_to_the_end_of_it` between ticks
}

/// This test has [a link_with_underscores][chunked-example] inside it. See #823.
/// See also [the issue tracker](https://github.com/Manishearth/rust-clippy/search?q=doc_markdown&type=Issues)
/// on GitHub (which is a camel-cased word, but is OK). And here is another [inline link][inline_link].
/// It can also be [inline_link2].
///
/// [chunked-example]: https://en.wikipedia.org/wiki/Chunked_transfer_encoding#Example
/// [inline_link]: https://foobar
/// [inline_link2]: https://foobar

/// The `main` function is the entry point of the program. Here it only calls the `foo_bar` and
/// `multiline_ticks` functions.
///
/// expression of the type  `_ <bit_op> m <cmp_op> c` (where `<bit_op>`
/// is one of {`&`, '|'} and `<cmp_op>` is one of {`!=`, `>=`, `>` ,
/// be_sure_we_got_to_the_end_of_it
fn main() {
//~^ ERROR: you should put `inline_link2` between ticks
//~| ERROR: you should put `link_with_underscores` between ticks
//~| ERROR: you should put `be_sure_we_got_to_the_end_of_it` between ticks
    foo_bar();
    multiline_ticks();
    test_emphasis();
    test_units();
}

use proc_macro::TokenStream;

#[proc_macro]
pub fn let_bcb(item: TokenStream) -> TokenStream {
    format!("let bcb{} = graph::BasicCoverageBlock::from_usize({}); let _ = {};", item, item, item)
        .parse()
        .unwrap()
}

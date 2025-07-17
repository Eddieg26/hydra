use macro_utils::syn;
use proc_macro::TokenStream;

mod expand;

#[proc_macro_derive(Action, attributes(name))]
pub fn derive_action(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    expand::expand_derive_action(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

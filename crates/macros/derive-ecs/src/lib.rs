use macro_utils::syn::{self};
use proc_macro::TokenStream;

mod event;

#[proc_macro_derive(Event)]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    event::expand_derive_event(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

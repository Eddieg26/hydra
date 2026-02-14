use macro_utils::syn::{self};
use proc_macro::TokenStream;

mod expand;

#[proc_macro_derive(Reflect)]
pub fn derive_reflect(input: proc_macro::TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    expand::expand_reflect(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

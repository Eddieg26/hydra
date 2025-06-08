use encase_derive_impl::implement;
use macro_utils::{
    syn::{self},
    workspace::get_crate_path,
};
use proc_macro::TokenStream;

mod expand;

#[proc_macro_derive(AsBinding, attributes(uniform, texture, sampler))]
pub fn derive_create_bind_group(input: proc_macro::TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    expand::expand_create_bind_group(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

implement!(get_crate_path("render").into());

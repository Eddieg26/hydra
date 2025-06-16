use encase_derive_impl::implement;
use macro_utils::{
    derive_marker, syn::{self}, workspace::get_crate_path
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

#[proc_macro_derive(RenderAsset)]
pub fn derive_render_asset(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "render", "RenderAsset")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

implement!(get_crate_path("render").into());

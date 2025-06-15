use macro_utils::{derive_marker, syn};
mod expand;

#[proc_macro_derive(Asset, attributes(reload, unload, dependency))]
pub fn derive_asset(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    match expand::expand_derive_asset(&mut input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_derive(AssetDependency, attributes(dependency))]
pub fn derive_asset_dependency(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    match expand::expand_derive_asset_dependency(&mut input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_derive(Settings)]
pub fn derive_settings(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "asset", "Settings")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

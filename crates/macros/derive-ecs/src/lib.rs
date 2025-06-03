use macro_utils::{
    derive_marker,
    syn::{self},
};
use proc_macro::TokenStream;

mod component;
mod plugin;

#[proc_macro_derive(Event)]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "ecs", "Event")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "ecs", "Component")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "ecs", "Resource")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(WorldMode)]
pub fn derive_mode(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "ecs", "WorldMode")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(AppTag)]
pub fn derive_app_tag(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "ecs", "AppTag")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Phase)]
pub fn derive_phase(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_marker(&mut input, "ecs", "Phase")
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(ComponentKit)]
pub fn derive_component_kit(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    component::expand_derive_component_kit(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(PluginKit)]
pub fn derive_plugin_kit(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as syn::DeriveInput);
    plugin::expand_derive_plugin_kit(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

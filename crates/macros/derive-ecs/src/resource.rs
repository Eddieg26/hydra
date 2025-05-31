use macro_utils::{proc_macro2, quote, syn, workspace::get_crate_path};

pub fn expand_derive_resource(
    input: &mut syn::DeriveInput,
) -> syn::Result<proc_macro2::TokenStream> {
    let ecs_crate = get_crate_path("ecs");

    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let expanded = quote::quote! {
        impl #impl_generics #ecs_crate::Resource for #name #type_generics #where_clause {}
    };

    Ok(expanded)
}

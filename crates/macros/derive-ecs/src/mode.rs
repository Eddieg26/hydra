use macro_utils::{proc_macro2::TokenStream, quote, syn, workspace::get_crate_path};

pub fn expand_derive_mode(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let ecs_crate = get_crate_path("ecs");

    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    Ok(TokenStream::from(quote::quote! {
        impl #impl_generics #ecs_crate::WorldMode for #name #type_generics #where_clause {}
    }))
}

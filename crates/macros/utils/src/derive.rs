use crate::workspace::get_crate_path;

pub fn derive_marker(
    input: &mut syn::DeriveInput,
    path: &'static str,
    name: &'static str,
) -> syn::Result<proc_macro2::TokenStream> {
    let marker_crate = get_crate_path(path);
    let name = syn::Ident::new(name, proc_macro2::Span::call_site());
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let expanded = quote::quote! {
        impl #impl_generics #marker_crate::#name for #ident #type_generics #where_clause {}
    };

    Ok(expanded)
}

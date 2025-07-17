use macro_utils::{ParseExp, proc_macro2::TokenStream, quote, syn, workspace::get_crate_path};

pub fn expand_derive_action(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let ecs_crate = get_crate_path("ecs");
    let input_crate = get_crate_path("input");
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let ident = &input.ident;
    let mut action_name = input.ident.to_string();

    if let Some(attribute) = input.attrs.first() {
        let meta = attribute.meta.require_name_value()?;
        action_name = meta.value.parse_str()?;
    }

    Ok(quote::quote! {
        impl #impl_generics #input_crate::Action for #ident #type_generics #where_clause {
            const NAME: &'static str = #action_name;
        }

        impl #impl_generics #ecs_crate::Event for #ident #type_generics #where_clause {

        }
    })
}

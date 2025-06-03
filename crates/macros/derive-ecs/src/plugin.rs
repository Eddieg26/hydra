use macro_utils::{
    proc_macro2::TokenStream,
    quote::quote,
    syn::{self, DeriveInput},
    workspace::get_crate_path,
};

pub fn expand_derive_plugin_kit(input: &mut DeriveInput) -> syn::Result<TokenStream> {
    let ecs_crate = get_crate_path("ecs");
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let fields = match &input.data {
        syn::Data::Struct(s) => s.fields.members(),
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "PluginKit can only be derived for structs",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics #ecs_crate::PluginKit for #name #type_generics #where_clause {
            fn get<P: #ecs_crate::PluginCollection>(self, plugins: &mut P) {
                #(plugins.add_plugin(self.#fields);)*
            }
        }
    })
}

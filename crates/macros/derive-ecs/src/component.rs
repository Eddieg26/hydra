use macro_utils::{
    proc_macro2::TokenStream,
    quote::quote,
    syn::{self, DeriveInput},
    workspace::get_crate_path,
};

pub fn expand_derive_component_kit(input: &mut DeriveInput) -> syn::Result<TokenStream> {
    let ecs_crate = get_crate_path("ecs");
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let fields = match &input.data {
        syn::Data::Struct(s) => &s.fields,
        _ => {
            return Err(syn::Error::new_spanned(
                name,
                "ComponentKit can only be derived for structs",
            ));
        }
    };

    let get_fields = fields.members();
    let ty_fields = fields.iter().map(|f| f.ty.clone()).collect::<Vec<_>>();

    Ok(quote! {
        impl #impl_generics #ecs_crate::ComponentKit for #name #type_generics #where_clause {
            fn ids(components: &mut #ecs_crate::Components) -> Vec<#ecs_crate::ComponentId> {
                let mut ids = Vec::new();
                #(ids.push(components.register::<#ty_fields>());)*
                ids
            }

            fn get<W: #ecs_crate::world::ComponentWriter>(self, writer: &mut W) {
                #(writer.write(self.#get_fields);)*
            }

            fn remove<R: #ecs_crate::world::ComponentRemover>(remover: &mut R) {
                #(remover.remove::<#ty_fields>();)*
            }
        }
    })
}

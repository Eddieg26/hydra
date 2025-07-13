use macro_utils::{
    Symbol,
    proc_macro2::TokenStream,
    quote::{self, format_ident, quote},
    syn::{self, Field, Variant},
    workspace::get_crate_path,
};

pub fn expand_derive_asset(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let asset_crate = get_crate_path("asset");
    let asset_name = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let dependencies = AssetDependencies::parse(&asset_crate, asset_name, input)?;

    Ok(quote::quote! {
        impl #impl_generics #asset_crate::Asset for #asset_name #type_generics #where_clause {
        }

        #dependencies
    })
}

pub fn expand_derive_asset_dependency(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let asset_crate = get_crate_path("asset");
    let asset_name = &input.ident;

    let dependencies = AssetDependencies::parse(&asset_crate, asset_name, input)?;

    Ok(quote::quote! { #dependencies })
}

struct AssetDependencies;

impl AssetDependencies {
    const DEPENDENCY: Symbol = Symbol::new("dependency");

    fn parse(
        crate_name: &syn::Ident,
        asset_name: &syn::Ident,
        input: &syn::DeriveInput,
    ) -> syn::Result<TokenStream> {
        let data = &input.data;
        let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

        let body = match data {
            syn::Data::Struct(data) => {
                let body = data
                    .fields
                    .iter()
                    .filter(|f| Self::is_dep_attr(f))
                    .enumerate()
                    .map(|(index, field)| match field.ident.as_ref() {
                        Some(ident) => {
                            quote!(#crate_name::AssetDependencies::get_dependencies(&self.#ident, reader); )
                        }
                        None => {
                            let index = syn::Index::from(index);
                            quote!(#crate_name::AssetDependencies::get_dependencies(&self.#index, reader); )
                        }
                    });

                quote!( #(#body)* )
            }
            syn::Data::Enum(data) => {
                let variants = data
                    .variants
                    .iter()
                    .filter(|v| Self::variant_has_dep(v))
                    .collect::<Vec<_>>();

                let cases = variants.iter().map(|v| {
                    let ident = &v.ident;
                    let body = v
                        .fields
                        .iter()
                        .filter(|f| Self::is_dep_attr(f))
                        .enumerate()
                        .map(|(i, field)| match field.ident.as_ref() {
                            Some(ident) => {
                                quote!(#crate_name::AssetDependencies::get_dependencies(#ident, reader);)
                            }
                            None => {
                                let index = format_ident!("field_{i}");
                                quote!(#crate_name::AssetDependencies::get_dependencies(#index, reader);)
                            }
                        });

                    let fields = match &v.fields {
                        syn::Fields::Named(fields) => {
                            let named = fields.named.iter().map(|f| f.ident.as_ref());
                            quote! { { #(#named,)* .. } }
                        }
                        syn::Fields::Unnamed(fields) => {
                            let unnamed =
                                (0..fields.unnamed.len()).map(|i| format_ident!("field_{i}"));
                            quote! { ( #(#unnamed,)* ) }
                        }
                        syn::Fields::Unit => {
                            unreachable!("Unit variants should not have dependencies")
                        }
                    };

                    quote! {
                        Self::#ident #fields => {
                            #(#body)*
                        }
                    }
                });

                if variants.is_empty() {
                    quote! {}
                } else {
                    quote!(match self { #(#cases)*, _ => {} })
                }
            }
            syn::Data::Union(data) => {
                return Err(syn::Error::new(
                    data.union_token.span,
                    "Union types are not supported for AssetDependencies",
                ));
            }
        };

        let reader = match body.is_empty() {
            true => quote! { _reader },
            false => quote! { reader },
        };

        Ok(quote::quote! {
            impl #impl_generics #crate_name::AssetDependencies for #asset_name #type_generics #where_clause {
                fn get_dependencies(&self, #reader: impl FnMut(#crate_name::ErasedId)) {
                    #body
                }
            }
        })
    }

    fn is_dep_attr(field: &Field) -> bool {
        field
            .attrs
            .iter()
            .any(|a| a.path().is_ident(&Self::DEPENDENCY))
    }

    fn variant_has_dep(variant: &Variant) -> bool {
        variant.fields.iter().any(Self::is_dep_attr)
    }
}

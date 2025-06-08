use macro_utils::{
    Symbol,
    proc_macro2::TokenStream,
    quote::{self, format_ident, quote},
    syn::{self, Attribute, Field, Variant, spanned::Spanned},
    workspace::get_crate_path,
};

pub fn expand_derive_asset(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let asset_crate = get_crate_path("asset");
    let asset_name = &input.ident;
    let (impl_generics, type_generics, where_clause) = &input.generics.split_for_impl();

    let mut attributes = input.attrs.iter();
    let action = match attributes.next().map(AssetAction::parse) {
        Some(Some(action)) => quote::quote! { Some(#asset_crate::#action) },
        _ => quote::quote! { None },
    };

    if attributes.next().is_some() {
        return Err(syn::Error::new(
            input.span(),
            "Only one of 'reload' or 'unload' attributes is allowed",
        ));
    }

    let dependencies = AssetDependency::parse(&asset_crate, asset_name, input)?;

    Ok(quote::quote! {
        impl #impl_generics #asset_crate::Asset for #asset_name #type_generics #where_clause {
            const DEPENDENCY_UNLOAD_ACTION: Option<#asset_crate::AssetAction> = #action;
        }

        #dependencies
    })
}

pub fn expand_derive_asset_dependency(input: &mut syn::DeriveInput) -> syn::Result<TokenStream> {
    let asset_crate = get_crate_path("asset");
    let asset_name = &input.ident;

    let dependencies = AssetDependency::parse(&asset_crate, asset_name, input)?;

    Ok(quote::quote! { #dependencies })
}

struct AssetAction;

impl AssetAction {
    const RELOAD: Symbol = Symbol::new("reload");
    const UNLOAD: Symbol = Symbol::new("unload");

    fn parse(attribute: &Attribute) -> Option<TokenStream> {
        if attribute.path().is_ident(&Self::RELOAD) {
            Some(quote::quote! { AssetAction::Reload})
        } else if attribute.path().is_ident(&Self::UNLOAD) {
            Some(quote::quote! { AssetAction::Unload})
        } else {
            None
        }
    }
}

struct AssetDependency;

impl AssetDependency {
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
                            quote!(#crate_name::AssetDependency::get(&self.#ident, reader); )
                        }
                        None => {
                            let index = syn::Index::from(index);
                            quote!(#crate_name::AssetDependency::get(&self.#index, reader); )
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
                                quote!(#crate_name::AssetDependency::get(#ident, reader);)
                            }
                            None => {
                                let index = format_ident!("field_{i}");
                                quote!(#crate_name::AssetDependency::get(#index, reader);)
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
                    "Union types are not supported for AssetDependency",
                ));
            }
        };

        let reader = match body.is_empty() {
            true => quote! { _reader },
            false => quote! { reader },
        };

        Ok(quote::quote! {
            impl #impl_generics #crate_name::AssetDependency for #asset_name #type_generics #where_clause {
                fn get<R: #crate_name::AssetDependencyReader>(&self, #reader: &mut R) {
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

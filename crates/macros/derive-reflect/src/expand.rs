use macro_utils::{
    proc_macro2::{self, Span, TokenStream},
    quote::{ToTokens, quote},
    syn::{self, Data, DataEnum, DataStruct, DeriveInput, Error, Fields, Ident, Result, Type},
    workspace::get_crate_path,
};
use std::borrow::Cow;

pub fn expand_reflect(input: &mut DeriveInput) -> Result<TokenStream> {
    let crate_path = get_crate_path("reflector_core");
    let name = &input.ident;

    let expanded = match &input.data {
        Data::Struct(data) => derive_struct(&crate_path, name, data),
        Data::Enum(data) => derive_enum(&crate_path, name, data),
        _ => return Err(Error::new(
            Span::call_site(),
            "Reflect can only be derived for structs",
        )),
    };

    Ok(TokenStream::from(quote! {
        #expanded
    }))
}

#[derive(Clone)]
struct Field<'a> {
    name: Cow<'a, Ident>,
    ty: &'a Type,
    index: Option<usize>, // For tuple struct fields
}

impl<'a> ToTokens for Field<'a> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = self.name.as_ref();
        tokens.extend::<proc_macro2::TokenStream>(quote! { stringify!(#name) }.into());
    }
}

fn get_fields<'a>(fields: &'a Fields) -> Vec<Field<'a>> {
    let fields: Vec<_> = match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .filter_map(|f| {
                f.ident.as_ref().map(|name| Field {
                    name: Cow::Borrowed(name),
                    ty: &f.ty,
                    index: None,
                })
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .filter_map(|(i, f)| match f.ident.as_ref() {
                Some(name) => Some(Field {
                    name: Cow::Borrowed(name),
                    ty: &f.ty,
                    index: Some(i),
                }),
                None => Some(Field {
                    name: Cow::Owned(Ident::new(&format!("field_{i}"), Span::call_site())),
                    ty: &f.ty,
                    index: Some(i),
                }),
            })
            .collect(),
        Fields::Unit => vec![],
    };

    fields
}

fn reflect_fields<'a>(fields: &'a [Field<'a>]) -> proc_macro2::TokenStream {
    let f = fields
        .iter()
        .map(|field| {
            let name = field.name.as_ref();
            let (field_name, accessor) = if let Some(index) = field.index {
                let idx = syn::Index::from(index);
                let name_str = proc_macro2::Literal::string(&index.to_string());
                (quote! { #name_str }, quote! { self.#idx })
            } else {
                (quote! { stringify!(#name) }, quote! { self.#name })
            };
            quote! {ReflectValue {
                    name: #field_name,
                    value: #accessor.reflect(),
            }}
        })
        .collect::<Vec<_>>();

    quote! {vec![
        #(#f),*
    ]}
}

fn default_fields<'a>(fields: &'a [Field<'a>]) -> proc_macro2::TokenStream {
    let f = fields
        .iter()
        .map(|field| {
            let ty = field.ty;
            let name = field.name.as_ref();
            let field_name = if let Some(index) = field.index {
                let name_str = proc_macro2::Literal::string(&index.to_string());
                quote! { #name_str }
            } else {
                quote! { stringify!(#name) }
            };
            quote! {ReflectValue {
                    name: #field_name,
                    value: #ty::default().reflect(),
            }}
        })
        .collect::<Vec<_>>();

    quote! {vec![
        #(#f),*
    ]}
}

fn set_fields<'a>(fields: &'a [Field<'a>]) -> Vec<proc_macro2::TokenStream> {
    fields
        .iter()
        .map(|field| {
            let name = field.name.as_ref();
            let (field_name, accessor) = if let Some(index) = field.index {
                let idx = syn::Index::from(index);
                let name_str = proc_macro2::Literal::string(&index.to_string());
                (quote! { #name_str }, quote! { self.#idx })
            } else {
                (quote! { stringify!(#name) }, quote! { self.#name })
            };
            quote! {
                #field_name => {
                    #accessor.set_reflect(field.value)?;
                }
            }
        })
        .collect::<Vec<_>>()
}

fn define_fields<'a>(fields: &'a [Field<'a>]) -> proc_macro2::TokenStream {
    let f = fields
        .iter()
        .map(|field| {
            let Field {
                name, ty, index, ..
            } = field;
            let field_name = if let Some(idx) = index {
                let name_str = proc_macro2::Literal::string(&idx.to_string());
                quote! { #name_str }
            } else {
                quote! { stringify!(#name) }
            };
            quote! {
                let #name = get_field::<#ty>(#field_name, &mut value);
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #(#f),*
    }
}

fn derive_struct<'a>(
    path: &'a Ident,
    name: &'a Ident,
    data: &'a DataStruct,
) -> proc_macro2::TokenStream {
    let fields = get_fields(&data.fields);
    let reflect_fields = reflect_fields(&fields);
    let set_fields = set_fields(&fields);

    quote! {
        impl #path::Reflect for #name {
            fn type_name(&self) -> &'static str {
                stringify!(#name)
            }

            fn reflect(&self) -> #path::ReflectType {
                use #path::{ReflectType, ReflectObject, ReflectValue, ReflectError};

                ReflectType::Object(#reflect_fields)
            }

            fn set_reflect(&mut self, value: #path::ReflectType) -> Result<(), #path::ReflectError> {
                use #path::{ReflectType, ReflectObject, ReflectValue, ReflectError};

                match value {
                    ReflectType::Object(fields) => {
                        for field in fields {
                            match field.name {
                                #(#set_fields)*
                                _ => return Err(ReflectError::UnknownField {
                                    name: String::from("unknown"),
                                }),
                            }
                        }
                        Ok(())
                    }
                    _ => Err(ReflectError::InvalidType {
                        name: self.type_name(),
                        expected: stringify!(#name),
                    }),
                }
            }
        }
    }
}

fn derive_enum<'a>(
    path: &'a Ident,
    name: &'a Ident,
    data: &'a DataEnum,
) -> proc_macro2::TokenStream {
    let mut variants = Vec::with_capacity(data.variants.len());
    let mut default_values = Vec::with_capacity(data.variants.len());
    let mut match_values = Vec::with_capacity(data.variants.len());

    for variant in &data.variants {
        let name = &variant.ident;
        let fields = get_fields(&variant.fields);
        let defaults = default_fields(&fields);
        let defined = define_fields(&fields);
        let field_names = fields.iter().map(|f| f.name.as_ref()).collect::<Vec<_>>();

        let default = match &variant.fields {
            Fields::Named(_) => {
                let field_reflect = fields.iter().map(|f| {
                    let fname = f.name.as_ref();
                    quote! {
                        ReflectValue {
                            name: stringify!(#fname),
                            value: #fname.reflect(),
                        }
                    }
                });
                quote! {
                    Self::#name {#(#field_names),*} => {
                        ReflectObject {
                            name: stringify!(#name),
                            fields: vec![#(#field_reflect),*]
                        }
                    }
                }
            }
            Fields::Unnamed(_) => {
                let field_reflect = fields.iter().enumerate().map(|(i, f)| {
                    let fname = f.name.as_ref();
                    let name_str = proc_macro2::Literal::string(&i.to_string());
                    quote! {
                        ReflectValue {
                            name: #name_str,
                            value: #fname.reflect(),
                        }
                    }
                });
                quote! {
                    Self::#name(#(#field_names),*) => {
                        ReflectObject {
                            name: stringify!(#name),
                            fields: vec![#(#field_reflect),*]
                        }
                    }
                }
            }
            Fields::Unit => quote! {
                Self::#name => {
                    ReflectObject {
                        name: stringify!(#name),
                        fields: vec![]
                    }
                }
            },
        };

        let match_value = match &variant.fields {
            Fields::Named(_) => quote! { Self::#name {#(#field_names),*} },
            Fields::Unnamed(_) => quote! { Self::#name (#(#field_names),*) },
            Fields::Unit => quote! { Self::#name },
        };

        default_values.push(default);
        match_values.push(quote! {
            stringify!(#name) => {
               #defined

                #match_value
            }
        });
        variants.push(quote! { ReflectObject {
            name: stringify!(#name),
            fields: #defaults,
        }});
    }

    match_values.push(quote! {
        _ => return Ok(())
    });

    quote! {
        impl #path::Reflect for #name {
            fn type_name(&self) -> &'static str {
                stringify!(#name)
            }

            fn reflect(&self) -> #path::ReflectType {
                use #path::{Reflect, ReflectType, ReflectObject, ReflectValue, ReflectError};

                let variants = vec![
                    #(#variants),*
                ];

                let value = match self {
                    #(#default_values),*
                };

                ReflectType::Enum { value: Box::new(value), variants }
            }

            fn set_reflect(&mut self, value: #path::ReflectType) -> Result<(), #path::ReflectError> {
                use #path::{Reflect, ReflectType, ReflectObject, ReflectValue, ReflectError};

                fn get_field<R: Reflect>(name: &'static str, obj: &mut ReflectObject) -> R {
                    let index = obj.fields.iter().position(|f| f.name == name);
                    let new_value = index
                        .map(|i| obj.fields.remove(i).value)
                        .unwrap_or_else(|| R::default().reflect());
                    R::construct(new_value)
                }

                match value {
                    ReflectType::Enum { mut value, ..} => {
                        let new_value = match value.name {
                            #(#match_values),*
                        };

                        *self = new_value;
                        Ok(())
                    }
                    v => Err(ReflectError::InvalidType {
                        name: v.type_name(),
                        expected: stringify!(#name),
                    }),
                }
            }
        }
    }
}

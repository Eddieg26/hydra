use macro_utils::{
    ParseExp, Symbol,
    proc_macro2::{Span, TokenStream},
    quote::{ToTokens, quote},
    syn::{
        self, DeriveInput, Error, Ident, Meta, Result, parse::Parse, punctuated::Punctuated,
        token::Comma,
    },
    workspace::get_crate_path,
};
use std::collections::HashMap;

/*
[uniform(0)]
[storage(0, read_write, visibility="compute")]
[texture(0, dimension=2d, visibility="fragment")]
[sampler(0, sampler_type="uint", visibility="vertex, fragment")]
*/

#[allow(non_snake_case)]
pub fn expand_create_bind_group(input: &mut DeriveInput) -> Result<TokenStream> {
    let LAYOUTS = Ident::new("layouts", Span::call_site());
    let BINDINGS = Ident::new("bindings", Span::call_site());
    let TEXTURES = Ident::new("textures", Span::call_site());
    let FALLBACKS = Ident::new("fallbacks", Span::call_site());

    let ecs = get_crate_path("ecs");
    let render = get_crate_path("render");
    let name = &input.ident;
    let data = match &input.data {
        syn::Data::Struct(data) => data,
        syn::Data::Enum(_) => return Err(Error::new_spanned(&input, "Enums are not supported")),
        syn::Data::Union(_) => return Err(Error::new_spanned(&input, "Unions are not supported")),
    };

    let fields = match &data.fields {
        syn::Fields::Named(fields) => Ok(&fields.named),
        syn::Fields::Unnamed(_) => Err(Error::new_spanned(
            &data.fields,
            "Unnamed fields are not supported",
        )),
        syn::Fields::Unit => Err(Error::new_spanned(
            &data.fields,
            "Unit structs are not supported",
        )),
    }?;

    let mut uniforms = HashMap::new();
    let mut layouts = TokenStream::new();
    let mut bindings = TokenStream::new();

    for field in fields {
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };

        for attr in &field.attrs {
            if !BindingType::validate(attr.path()) {
                continue;
            }

            let binding_meta = attr.parse_args_with(BindingMeta::parse)?;
            let binding = binding_meta.binding;

            match BindingType::parse(attr.path(), &binding_meta)? {
                BindingType::Uniform => uniforms
                    .entry(binding_meta.binding)
                    .or_insert(Vec::new())
                    .push((ident, &field.ty)),
                BindingType::Storage {
                    read_only,
                    visibility,
                } => {
                    layouts.extend(quote! {
                        #LAYOUTS.with_storage(#binding, #visibility, false, #read_only, None, None);
                    });

                    bindings.extend(quote! {
                        let buffer = StorageBuffer::new(device, self.#ident, None, None);
                        #BINDINGS.with_storage(#binding, &buffer, 0, None);
                    });
                }
                BindingType::Texture {
                    dimension,
                    sample_ty,
                    visibility,
                } => {
                    let view_dimension = dimension.view_tokens();
                    layouts.extend(quote! {
                        #LAYOUTS.with_texture(#binding, #visibility, #view_dimension, #sample_ty);
                    });

                    bindings.extend(quote! {
                        let texture = self.#ident.into_optional_id()
                            .and_then(|id| #TEXTURES.get(&id))
                            .unwrap_or_else(|| #FALLBACKS.texture(#dimension));

                        #BINDINGS.with_texture(#binding, texture.view());
                    });
                }
                BindingType::Sampler { ty, visibility } => {
                    layouts.extend(quote! {
                        #LAYOUTS.with_sampler(#binding, #visibility, #ty);
                    });

                    bindings.extend(quote! {
                        let sampler = self.#ident.into_optional_id()
                            .and_then(|id| #TEXTURES.get(&id))
                            .map(|t| t.sampler())
                            .unwrap_or_else(|| &#FALLBACKS.sampler);

                        #BINDINGS.with_sampler(#binding, sampler);
                    });
                }
            }
        }
    }

    for (binding, mut fields) in uniforms {
        let (fields, types): (Vec<_>, Vec<_>) = fields.drain(..).unzip();
        let struct_name = syn::Ident::new(
            &format!("{name}UniformBufferBindingData{binding}"),
            Span::call_site(),
        );

        layouts.extend(quote! {
            #LAYOUTS.with_uniform(#binding, ShaderStages::all(), false, None, None);
        });

        bindings.extend(quote! {
            #[derive(ShaderType, Clone, Copy)]
            struct #struct_name {
                #(#fields: #types),*
            }

            let buffer = UniformBuffer::new(device, #struct_name { #(#fields: self.#fields),* }, None, None);
            #BINDINGS.with_uniform(#binding, &buffer, 0, None);
        });
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let label = name.to_string();

    let tokens = quote! {
        impl #impl_generics #render::resources::AsBinding for #name #ty_generics #where_clause {
            type Arg = (
                #ecs::unlifetime::Read<#render::RenderAssets<#render::GpuTexture>>,
                #ecs::unlifetime::Read<#render::Fallbacks>,
            );

            fn label() -> Option< &'static str> {
                Some(#label)
            }

            fn create_bind_group(
                &self,
                device: &#render::RenderDevice,
                layout: &#render::BindGroupLayout,
                arg: &#ecs::system::ArgItem<Self::Arg>
            ) -> Result<#render::BindGroup, #render::CreateBindGroupError> {
                use #render::{BindGroupBuilder, OptionalAssetId, uniform::UniformBuffer, storage::StorageBuffer, GpuTexture, ShaderType, TextureDimension, wgpu::TextureViewDimension};

                let (#TEXTURES, #FALLBACKS) = arg;
                let mut #BINDINGS = BindGroupBuilder::new(layout);

                #bindings

               Ok(#BINDINGS.build(device))
            }

            fn create_bind_group_layout(device: &#render::RenderDevice)  -> #render::BindGroupLayout {
                use #render::{BindGroupLayoutBuilder, wgpu::{TextureSampleType, TextureViewDimension, SamplerBindingType, ShaderStages}};

                let mut #LAYOUTS = BindGroupLayoutBuilder::new();

                #layouts

                #LAYOUTS.build(device)
            }
        }
    };

    Ok(tokens)
}

struct BindingMeta {
    binding: u32,
    meta: Punctuated<Meta, Comma>,
}

impl syn::parse::Parse for BindingMeta {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let binding: u32 = input.parse::<syn::LitInt>()?.base10_parse()?;
        let _ = input.parse::<Comma>();

        let meta = match input.parse_terminated(Meta::parse, Comma) {
            Ok(meta) => meta,
            Err(_) => Punctuated::new(),
        };

        Ok(Self { binding, meta })
    }
}

enum BindingType {
    Uniform,
    Storage {
        read_only: bool,
        visibility: Visibility,
    },
    Texture {
        dimension: TextureDimension,
        sample_ty: TextureSampleType,
        visibility: Visibility,
    },
    Sampler {
        ty: SamplerType,
        visibility: Visibility,
    },
}

impl BindingType {
    const UNIFORM: Symbol = Symbol::new("uniform");
    const STORAGE: Symbol = Symbol::new("storage");
    const TEXTURE: Symbol = Symbol::new("texture");
    const SAMPLER: Symbol = Symbol::new("sampler");

    fn parse(path: &syn::Path, meta: &BindingMeta) -> Result<Self> {
        if path == Self::UNIFORM {
            Ok(Self::Uniform)
        } else if path == Self::STORAGE {
            const READ_ONLY: Symbol = Symbol::new("read_only");

            let mut read_only = false;
            let mut visibility = Visibility::empty();

            for meta in &meta.meta {
                match meta {
                    Meta::List(meta) if meta.path == Visibility::PATH => {
                        visibility = Visibility::parse(meta, Visibility::empty())?;
                    }
                    Meta::NameValue(meta) if meta.path == READ_ONLY => {
                        read_only = meta.value.parse_bool()?;
                    }
                    _ => continue,
                }
            }

            Ok(Self::Storage {
                read_only,
                visibility,
            })
        } else if path == Self::TEXTURE {
            let mut dimension = TextureDimension::D2;
            let mut sample_ty = TextureSampleType::Float { filterable: false };
            let mut visibility = Visibility::empty();

            for meta in &meta.meta {
                match meta {
                    Meta::List(meta) if meta.path == Visibility::PATH => {
                        visibility = Visibility::parse(meta, Visibility::FRAGMENT)?;
                    }
                    Meta::NameValue(meta) if meta.path == TextureDimension::PATH => {
                        dimension = TextureDimension::parse(meta)?
                    }
                    Meta::NameValue(meta) if meta.path == TextureSampleType::PATH => {
                        sample_ty = TextureSampleType::parse(meta)?
                    }
                    _ => continue,
                }
            }

            Ok(Self::Texture {
                dimension,
                sample_ty,
                visibility,
            })
        } else if path == Self::SAMPLER {
            let mut ty = SamplerType::NonFiltering;
            let mut visibility = Visibility::empty();

            for meta in &meta.meta {
                match meta {
                    Meta::List(meta) if meta.path == Visibility::PATH => {
                        visibility = Visibility::parse(meta, Visibility::FRAGMENT)?;
                    }
                    Meta::NameValue(meta) if meta.path == SamplerType::PATH => {
                        ty = SamplerType::parse(meta)?
                    }
                    _ => continue,
                }
            }

            Ok(Self::Sampler { ty, visibility })
        } else {
            Err(syn::Error::new_spanned(
                path,
                format!("Unknow attribute: {:?}", path),
            ))
        }
    }

    fn validate(path: &syn::Path) -> bool {
        let v = path == Self::UNIFORM
            || path == Self::STORAGE
            || path == Self::TEXTURE
            || path == Self::SAMPLER;

        return v;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextureDimension {
    D1,
    D2,
    D3,
    Cube,
}

impl TextureDimension {
    const PATH: Symbol = Symbol::new("dimension");
    const D_1: &str = "d1";
    const D_2: &str = "d2";
    const D_3: &str = "d3";
    const CUBE: &str = "cube";

    fn parse(meta: &syn::MetaNameValue) -> Result<Self> {
        match meta.value.parse_str().as_deref() {
            Ok(Self::D_1) => Ok(Self::D1),
            Ok(Self::D_2) => Ok(Self::D2),
            Ok(Self::D_3) => Ok(Self::D3),
            Ok(Self::CUBE) => Ok(Self::Cube),
            Err(e) => return Err(Error::new_spanned(meta, e)),
            _ => return Err(Error::new_spanned(meta, "unknown dimension")),
        }
    }

    fn view_tokens(&self) -> TokenStream {
        match self {
            TextureDimension::D1 => quote! { TextureViewDimension::D1 },
            TextureDimension::D2 => quote! { TextureViewDimension::D2 },
            TextureDimension::D3 => quote! { TextureViewDimension::D3 },
            TextureDimension::Cube => quote! { TextureViewDimension::Cube },
        }
    }
}

impl ToTokens for TextureDimension {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TextureDimension::D1 => tokens.extend(quote! { TextureDimension::D1 }),
            TextureDimension::D2 => tokens.extend(quote! { TextureDimension::D2 }),
            TextureDimension::D3 => tokens.extend(quote! { TextureDimension::D3 }),
            TextureDimension::Cube => tokens.extend(quote! { TextureDimension::Cube }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextureSampleType {
    Float { filterable: bool },
    Int,
    UInt,
    Depth,
}

impl TextureSampleType {
    const PATH: Symbol = Symbol::new("sample_ty");
    const FLOAT: &str = "float";
    const FLOAT_FILTER: &str = "float filter";
    const INT: &str = "int";
    const UINT: &str = "uint";
    const DEPTH: &str = "depth";

    fn parse(meta: &syn::MetaNameValue) -> Result<Self> {
        match meta.value.parse_str().as_deref() {
            Ok(Self::FLOAT) => Ok(Self::Float { filterable: false }),
            Ok(Self::FLOAT_FILTER) => Ok(Self::Float { filterable: true }),
            Ok(Self::INT) => Ok(Self::Int),
            Ok(Self::UINT) => Ok(Self::UInt),
            Ok(Self::DEPTH) => Ok(Self::Depth),
            Err(e) => Err(Error::new_spanned(meta, e)),
            _ => return Err(Error::new_spanned(meta, "unknown sampler type")),
        }
    }
}

impl ToTokens for TextureSampleType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TextureSampleType::Float { filterable } => {
                tokens.extend(quote! { TextureSampleType::Float { filterable: #filterable } })
            }
            TextureSampleType::Depth => tokens.extend(quote! { TextureSampleType::Depth }),
            TextureSampleType::Int => tokens.extend(quote! { TextureSampleType::SInt }),
            TextureSampleType::UInt => tokens.extend(quote! { TextureSampleType::UInt }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SamplerType {
    Filtering,    // Filterable true
    NonFiltering, // Filterable false, sint, uint
    Comparison,   // Depth
}

impl SamplerType {
    const PATH: Symbol = Symbol::new("ty");
    const FILTER: &str = "filtering";
    const NON_FILTER: &str = "non_filtering";
    const COMPARISON: &str = "comparision";

    fn parse(meta: &syn::MetaNameValue) -> Result<Self> {
        match meta.value.parse_str().as_deref() {
            Ok(Self::FILTER) => Ok(Self::Filtering),
            Ok(Self::NON_FILTER) => Ok(Self::NonFiltering),
            Ok(Self::COMPARISON) => Ok(Self::Comparison),
            Err(e) => Err(Error::new_spanned(meta, e)),
            _ => return Err(Error::new_spanned(meta, "unknown sampler type")),
        }
    }
}

impl ToTokens for SamplerType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            SamplerType::Filtering => tokens.extend(quote! { SamplerBindingType::Filtering }),
            SamplerType::NonFiltering => tokens.extend(quote! { SamplerBindingType::NonFiltering }),
            SamplerType::Comparison => tokens.extend(quote! { SamplerBindingType::Comparison }),
        }
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct Visibility: u32 {
        const VERTEX = 0b0001;
        const FRAGMENT = 0b0010;
        const COMPUTE = 0b0100;
    }
}

impl Visibility {
    const PATH: Symbol = Symbol::new("visibility");
    const VS: Symbol = Symbol::new("vertex");
    const FS: Symbol = Symbol::new("fragment");
    const CS: Symbol = Symbol::new("compute");
    const ALL: Symbol = Symbol::new("all");
    const NONE: Symbol = Symbol::new("none");

    fn parse(list: &syn::MetaList, default: Visibility) -> Result<Self> {
        let mut flags = Vec::new();
        list.parse_nested_meta(|meta| Ok(flags.push(meta.path)))?;

        if flags.is_empty() {
            return Err(Error::new_spanned(list, ""));
        } else if flags.len() == 1 {
            if flags[0] == Self::ALL {
                return Ok(Visibility::all());
            } else if flags[0] == Self::NONE {
                return Ok(Visibility::all());
            }
        }

        let mut visibility = default;

        for flag in flags {
            if flag == Self::VS {
                visibility |= Visibility::VERTEX;
            } else if flag == Self::FS {
                visibility |= Visibility::FRAGMENT;
            } else if flag == Self::CS {
                visibility |= Visibility::COMPUTE;
            } else {
                return Err(Error::new_spanned(flag, ""));
            }
        }

        Ok(visibility)
    }
}

impl ToTokens for Visibility {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut flags = vec![quote! {ShaderStages::empty()}];

        if self.contains(Self::VERTEX) {
            flags.push(quote! {ShaderStages::VERTEX})
        }

        if self.contains(Self::FRAGMENT) {
            flags.push(quote! {ShaderStages::FRAGMENT})
        }

        if self.contains(Self::COMPUTE) {
            flags.push(quote! {ShaderStages::COMPUTE})
        }

        tokens.extend(quote! { #(#flags)|* });
    }
}

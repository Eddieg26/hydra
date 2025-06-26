use macro_utils::{
    proc_macro2::{Span, TokenStream},
    quote::{ToTokens, quote},
    syn::{
        self, DeriveInput, Error, Expr, Meta, MetaNameValue, Result,
        parse::{Parse, ParseBuffer},
        punctuated::Punctuated,
        token::Comma,
    },
    workspace::get_crate_path,
};

/*
   [uniform(0)]
   [texture(0, dimension=2d, visibility="fragment")]
   [sampler(0, sampler_type="uint", visibility="vertex, fragment")]
*/

pub fn expand_create_bind_group(input: &mut DeriveInput) -> Result<TokenStream> {
    let render = get_crate_path("render");
    let ecs = get_crate_path("ecs");

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

    let mut uniform: Uniform = Uniform::new(name.to_string());
    let mut bindings = vec![];

    for attribute in &input.attrs {
        let Some(attribute_name) = attribute.path().get_ident() else {
            continue;
        };

        if attribute_name != "uniform" {
            continue;
        }

        uniform.index = attribute
            .parse_args_with(|input: &ParseBuffer| {
                input.parse::<syn::LitInt>()?.base10_parse::<u32>()
            })
            .ok();
    }

    for field in fields {
        let Some(field_name) = &field.ident else {
            continue;
        };

        for attribute in &field.attrs {
            let Some(attribute_name) = attribute.path().get_ident() else {
                continue;
            };

            if attribute_name == "uniform" {
                uniform.add_field(UniformField::new(field_name, &field.ty));
            } else if attribute_name == "texture" {
                let binding = attribute.parse_args_with(BindingType::parse_texture)?;
                bindings.push(Binding {
                    name: field_name,
                    ty: binding,
                });
            } else if attribute_name == "sampler" {
                let binding = attribute.parse_args_with(BindingType::parse_sampler)?;
                bindings.push(Binding {
                    name: field_name,
                    ty: binding,
                });
            }
        }
    }

    if uniform.fields.is_empty() && bindings.is_empty() {
        return Err(Error::new_spanned(&input, "No bindings found"));
    }

    if uniform.index.is_none() && !uniform.fields.is_empty() {
        return Err(Error::new_spanned(&input, "Uniform index is required"));
    }

    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let layout_def = LayoutDefinition::new(&uniform, &bindings);
    let binding_def = BindingDefinition::new(&uniform, &bindings);
    let ty_name = name.to_string();

    let tokens = quote! {
        impl #impl_generics #render::resources::AsBinding for #name #ty_generics #where_clause {
            type Arg = (
                #ecs::system::unlifetime::Read<#render::RenderAssets<#render::GpuTexture>>,
                #ecs::system::unlifetime::Read<#render::Fallbacks>,
            );

            fn label() -> Option< &'static str> {
                Some(#ty_name)
            }

            fn create_bind_group(
                &self,
                device: &#render::RenderDevice,
                layout: &#render::BindGroupLayout,
                arg: &#ecs::system::ArgItem<Self::Arg>
            ) -> Result<#render::BindGroup, #render::CreateBindGroupError> {
                use #render::{BindGroupBuilder, OptionalAssetId, uniform::UniformBuffer, GpuTexture, ShaderType, TextureDimension, wgpu::TextureViewDimension};
                let (textures, fallbacks) = arg;
                #binding_def
            }

            fn create_bind_group_layout(device: &#render::RenderDevice)  -> #render::BindGroupLayout {
                use #render::{BindGroupLayoutBuilder, wgpu::{TextureSampleType, TextureViewDimension, SamplerBindingType, ShaderStages}};
                #layout_def
            }
        }
    };

    Ok(tokens)
}

struct Uniform<'a> {
    index: Option<u32>,
    name: syn::Ident,
    fields: Vec<UniformField<'a>>,
}

impl<'a> Uniform<'a> {
    fn new(name: String) -> Self {
        Self {
            index: None,
            name: syn::Ident::new(&format!("{}_Uniform", &name), Span::call_site()),
            fields: Vec::new(),
        }
    }

    fn name(&self) -> &syn::Ident {
        &self.name
    }

    fn fields(&self) -> &[UniformField<'a>] {
        &self.fields
    }

    fn add_field(&mut self, field: UniformField<'a>) {
        self.fields.push(field);
    }
}

struct UniformField<'a> {
    name: &'a syn::Ident,
    ty: &'a syn::Type,
}

impl<'a> UniformField<'a> {
    fn new(name: &'a syn::Ident, ty: &'a syn::Type) -> Self {
        Self { name, ty }
    }
}

impl ToTokens for UniformField<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = self.name;
        let ty = self.ty;
        tokens.extend(quote! {
            pub #name: #ty,
        });
    }
}

struct LayoutDefinition<'a> {
    uniform: &'a Uniform<'a>,
    bindings: &'a [Binding<'a>],
}

impl<'a> LayoutDefinition<'a> {
    fn new(uniform: &'a Uniform, bindings: &'a [Binding<'a>]) -> Self {
        Self { uniform, bindings }
    }
}

impl ToTokens for LayoutDefinition<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(quote! {
            let mut builder = BindGroupLayoutBuilder::new();
        });

        for binding in self.bindings {
            match binding.ty {
                BindingType::Texture {
                    index,
                    dimension,
                    sample_ty,
                    visibility,
                } => {
                    let dimension = dimension.to_view_tokens();
                    tokens.extend(quote! {
                        builder.with_texture(
                            #index,
                            ShaderStages::all(),
                            #dimension,
                            #sample_ty
                        );
                    });
                }
                BindingType::Sampler {
                    index,
                    ty,
                    visibility,
                } => {
                    tokens.extend(quote! {
                        builder.with_sampler(
                            #index,
                            ShaderStages::all(),
                            #ty
                        );
                    });
                }
            }
        }

        if let Some(index) = self.uniform.index {
            tokens.extend(quote! {
                builder.with_uniform(
                    #index,
                    ShaderStages::all(),
                    false,
                    None,
                    None,
                );
            });
        }

        tokens.extend(quote! {
            builder.build(device)
        });
    }
}

struct BindingDefinition<'a> {
    uniform: &'a Uniform<'a>,
    bindings: &'a [Binding<'a>],
}

impl<'a> BindingDefinition<'a> {
    fn new(uniform: &'a Uniform, bindings: &'a [Binding]) -> Self {
        Self { uniform, bindings }
    }
}

impl ToTokens for BindingDefinition<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(quote! {
            let mut builder = BindGroupBuilder::new(layout);
        });

        for binding in self.bindings {
            let name = binding.name;
            match binding.ty {
                BindingType::Texture {
                    index, dimension, ..
                } => {
                    tokens.extend(quote! {
                        let id = self.#name.into_optional_id();
                        let texture = match id.and_then(|id| textures.get(&id)) {
                            Some(texture) => texture,
                            None => fallbacks.texture(#dimension),
                        };

                        builder.with_texture(
                            #index,
                            texture.as_ref(),
                        );
                    });
                }
                BindingType::Sampler { index, .. } => {
                    tokens.extend(quote! {
                        let id = self.#name.into_optional_id();
                        let sampler = match id.and_then(|id| textures.get(&id)) {
                            Some(texture) => texture.sampler(),
                            None => &fallbacks.sampler,
                        };

                        builder.with_sampler(
                            #index,
                            sampler.as_ref(),
                        );
                    });
                }
            }
        }

        if let Some(index) = self.uniform.index {
            // Create struct definition for uniform and implement ShaderType for it
            // Create an object of the struct
            // Create a uniform buffer with the object
            // Add the uniform buffer to the bind group builder

            let struct_name = self.uniform.name();
            let fields = self.uniform.fields();
            let field_names = fields.iter().map(|field| field.name);
            tokens.extend(quote! {
                #[derive(ShaderType)]
                struct #struct_name {
                    #(#fields)*
                }

                let value = #struct_name {
                    #(#field_names: self.#field_names,)*
                };

                let buffer = UniformBuffer::new(device, &value, None, None);

                builder.with_uniform(
                    #index,
                    &buffer,
                    0,
                    None,
                );
            });
        }

        tokens.extend(quote! {
            Ok(builder.build(device))
        });
    }
}

bitflags::bitflags! {
    #[derive(Debug,  Clone, Copy, PartialEq, Eq)]
    struct Visibility: u32 {
        const VERTEX = 0b0001;
        const FRAGMENT = 0b0010;
        const COMPUTE = 0b0100;
    }
}

impl ToTokens for Visibility {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut parts = Vec::new();

        if self.contains(Visibility::VERTEX) {
            parts.push(quote! { ShaderStages::VERTEX });
        }

        if self.contains(Visibility::FRAGMENT) {
            parts.push(quote! { ShaderStages::FRAGMENT });
        }

        if self.contains(Visibility::COMPUTE) {
            parts.push(quote! { ShaderStages::COMPUTE });
        }

        let temp = parts
            .into_iter()
            .fold(quote! {ShaderStages::empty()}, |tokens, token| {
                quote! { #tokens | #token }
            });

        tokens.extend(temp);
    }
}

impl Visibility {
    fn parse(meta: &Meta) -> Result<Self> {
        match meta {
            Meta::NameValue(meta) => {
                let visibility_str = parse_syn_str(meta)?;
                let mut visibility = Visibility::empty();

                for part in visibility_str.split(',') {
                    match part.trim() {
                        "vertex" => visibility |= Visibility::VERTEX,
                        "fragment" => visibility |= Visibility::FRAGMENT,
                        "compute" => visibility |= Visibility::COMPUTE,
                        _ => return Err(Error::new_spanned(meta, "Invalid visibility")),
                    }
                }

                Ok(visibility)
            }
            _ => Err(Error::new_spanned(
                meta,
                "Expected identifier for attribute value",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextureDimension {
    D1,
    D2,
    D3,
    Cube,
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

impl TextureDimension {
    fn to_view_tokens(&self) -> TokenStream {
        match self {
            TextureDimension::D1 => quote! { TextureViewDimension::D1 },
            TextureDimension::D2 => quote! { TextureViewDimension::D2 },
            TextureDimension::D3 => quote! { TextureViewDimension::D3 },
            TextureDimension::Cube => quote! { TextureViewDimension::Cube },
        }
    }
}

impl TextureDimension {
    fn parse(meta: &Meta) -> Result<Self> {
        match meta {
            Meta::NameValue(meta) => {
                let dimension_str = parse_syn_str(meta)?;
                match dimension_str.as_str() {
                    "1d" => Ok(Self::D1),
                    "2d" => Ok(Self::D2),
                    "3d" => Ok(Self::D3),
                    "cube" => Ok(Self::Cube),
                    _ => Err(Error::new_spanned(meta, "Invalid texture dimension")),
                }
            }
            _ => Err(Error::new_spanned(
                meta,
                "Expected identifier for attribute value",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextureSampleType {
    Float { filterable: bool },
    Depth,
    Int,
    UInt,
}

impl ToTokens for TextureSampleType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            TextureSampleType::Float { filterable } => {
                tokens.extend(quote! { TextureSampleType::Float { filterable: #filterable } })
            }
            TextureSampleType::Depth => tokens.extend(quote! { TextureSampleType::Depth }),
            TextureSampleType::Int => tokens.extend(quote! { TextureSampleType::Int }),
            TextureSampleType::UInt => tokens.extend(quote! { TextureSampleType::UInt }),
        }
    }
}

impl TextureSampleType {
    fn parse(meta: &Meta) -> Result<Self> {
        match meta {
            Meta::NameValue(meta) => {
                let sample_ty_str = parse_syn_str(meta)?;
                match sample_ty_str.as_str() {
                    "float" => Ok(Self::Float { filterable: true }),
                    "depth" => Ok(Self::Depth),
                    "int" => Ok(Self::Int),
                    "uint" => Ok(Self::UInt),
                    _ => Err(Error::new_spanned(meta, "Invalid texture sample type")),
                }
            }
            _ => Err(Error::new_spanned(
                meta,
                "Expected identifier for attribute value",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SamplerType {
    Filtering,
    NonFiltering,
    Comparison,
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

impl SamplerType {
    fn parse(meta: &Meta) -> Result<Self> {
        match meta {
            Meta::NameValue(meta) => {
                let ty_str = parse_syn_str(meta)?;
                match ty_str.as_str() {
                    "filtering" => Ok(Self::Filtering),
                    "non_filtering" => Ok(Self::NonFiltering),
                    "comparison" => Ok(Self::Comparison),
                    _ => Err(Error::new_spanned(meta, "Invalid sampler type")),
                }
            }
            _ => Err(Error::new_spanned(
                meta,
                "Expected identifier for attribute value",
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BindingType {
    Texture {
        index: u32,
        dimension: TextureDimension,
        sample_ty: TextureSampleType,
        visibility: Visibility,
    },
    Sampler {
        index: u32,
        ty: SamplerType,
        visibility: Visibility,
    },
}

impl BindingType {
    fn parse_texture(input: syn::parse::ParseStream) -> Result<Self> {
        let binding: u32 = input.parse::<syn::LitInt>()?.base10_parse()?;
        let _ = input.parse::<Comma>();

        let meta = match input.parse_terminated(Meta::parse, Comma) {
            Ok(meta) => meta,
            Err(_) => Punctuated::new(),
        };

        let mut dimension = TextureDimension::D2;
        let mut sample_ty = TextureSampleType::Float { filterable: false };
        let mut visibility = Visibility::empty();

        for meta in meta {
            let name = meta.path().get_ident().ok_or_else(|| {
                Error::new_spanned(meta.path(), "Expected identifier for attribute name")
            })?;

            match name.to_string().as_str() {
                "dimension" => dimension = TextureDimension::parse(&meta)?,
                "sample_type" => sample_ty = TextureSampleType::parse(&meta)?,
                "visibility" => visibility = Visibility::parse(&meta)?,
                _ => return Err(Error::new_spanned(&meta, "Invalid attribute name")),
            }
        }

        Ok(Self::Texture {
            index: binding,
            dimension,
            sample_ty,
            visibility,
        })
    }

    fn parse_sampler(input: syn::parse::ParseStream) -> Result<Self> {
        let binding: u32 = input.parse::<syn::LitInt>()?.base10_parse()?;
        let _ = input.parse::<Comma>();

        let meta = match input.parse_terminated(Meta::parse, Comma) {
            Ok(meta) => meta,
            Err(_) => Punctuated::new(),
        };
        let mut ty = SamplerType::NonFiltering;
        let mut visibility = Visibility::empty();

        for meta in meta {
            let name = meta.path().get_ident().ok_or_else(|| {
                Error::new_spanned(meta.path(), "Expected identifier for attribute name")
            })?;

            match name.to_string().as_str() {
                "sampler_type" => ty = SamplerType::parse(&meta)?,
                "visibility" => visibility = Visibility::parse(&meta)?,
                _ => return Err(Error::new_spanned(&meta, "Invalid attribute name")),
            }
        }

        Ok(Self::Sampler {
            index: binding,
            ty,
            visibility,
        })
    }
}

struct Binding<'a> {
    name: &'a syn::Ident,
    ty: BindingType,
}

fn parse_syn_str(meta: &MetaNameValue) -> Result<String> {
    match &meta.value {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(lit),
            ..
        }) => Ok(lit.value()),
        _ => return Err(Error::new_spanned(&meta.path, "Expected string")),
    }
}

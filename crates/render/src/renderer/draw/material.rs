use crate::{
    AsBinding, BindGroup, BindGroupLayout, ExtractError, ExtractResource, RenderAsset,
    RenderDevice, RenderResource, Shader, view::View,
};
use asset::{Asset, AssetId};
use ecs::{
    Component, Resource,
    unlifetime::{Read, SCommands},
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum DepthWrite {
    Enabled,
    Disabled,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum BlendMode {
    Opaque,
    Transparent,
    Transmissive,
    AlphaMask,
}

impl Into<wgpu::BlendState> for BlendMode {
    fn into(self) -> wgpu::BlendState {
        match self {
            BlendMode::Opaque => wgpu::BlendState::REPLACE,
            BlendMode::Transparent => wgpu::BlendState::ALPHA_BLENDING,
            BlendMode::Transmissive => wgpu::BlendState::ALPHA_BLENDING,
            BlendMode::AlphaMask => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        }
    }
}

pub trait RenderPhase: 'static {
    type View: View;

    type Item: Default + Copy + Eq + Ord + Send + Sync + 'static;

    fn mode() -> BlendMode;
}

pub trait Material: Clone + Asset + AsBinding {
    type View: View;

    type Phase: RenderPhase<View = Self::View>;

    fn depth_write() -> DepthWrite {
        DepthWrite::Enabled
    }

    fn shader() -> impl Into<AssetId<Shader>>;
}

#[derive(Resource, Clone)]
pub struct MaterialLayout<M: Material>(BindGroupLayout, std::marker::PhantomData<M>);
impl<M: Material> std::ops::Deref for MaterialLayout<M> {
    type Target = BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M: Material> AsRef<BindGroupLayout> for MaterialLayout<M> {
    fn as_ref(&self) -> &BindGroupLayout {
        &self.0
    }
}

impl<M: Material> RenderResource for MaterialLayout<M> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::ArgItem<Self::Arg>) -> Result<Self, ExtractError<()>> {
        let layout = M::create_bind_group_layout(device);
        Ok(Self(layout, std::marker::PhantomData))
    }
}

#[derive(Clone)]
pub struct MaterialInstance<M: Material>(BindGroup, std::marker::PhantomData<M>);
impl<M: Material> std::ops::Deref for MaterialInstance<M> {
    type Target = BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M: Material> RenderAsset for MaterialInstance<M> {
    type Source = M;

    type Arg = (
        Read<RenderDevice>,
        Option<Read<MaterialLayout<M>>>,
        M::Arg,
        SCommands,
    );

    fn extract(
        asset: Self::Source,
        (device, layout, arg, commands): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Source>> {
        let layout = match layout {
            Some(layout) => layout.clone(),
            None => {
                commands.add(ExtractResource::<MaterialLayout<M>>::new());
                return Err(ExtractError::Retry(asset));
            }
        };

        let bind_group = asset
            .create_bind_group(device, &layout, arg)
            .map_err(|_| ExtractError::Retry(asset))?;

        Ok(Self(bind_group, std::marker::PhantomData))
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct MaterialRef<M: Material>(pub AssetId<M>);

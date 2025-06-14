use super::{RenderAssetExtractor, RenderResource};
use crate::{
    ExtractResource,
    device::RenderDevice,
    resources::{
        Shader,
        binding::{AsBinding, BindGroup, BindGroupLayout},
        extract::RenderAsset,
    },
};
use asset::{Asset, AssetId};
use ecs::{
    Resource,
    system::unlifetime::{Read, SCommands},
};
use transform::GlobalTransform;

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

pub enum DepthWrite {
    Off,
    On,
}

pub trait RenderItem:
    Copy + Clone + Eq + PartialEq + Ord + PartialEq + Send + Sync + Sized + 'static
{
    const SORT: bool = false;

    fn new(_: &GlobalTransform, _: &GlobalTransform) -> Self;
}

impl RenderItem for () {
    const SORT: bool = false;

    fn new(_: &GlobalTransform, _: &GlobalTransform) -> Self {
        ()
    }
}

pub trait RenderPhase: Send + Sync + 'static {
    type Item: RenderItem;

    fn mode() -> BlendMode;
    fn depth_write() -> DepthWrite {
        DepthWrite::On
    }
}

pub trait Material: Asset + AsBinding + Clone + Sized {
    type Phase: RenderPhase;

    fn shader() -> impl Into<AssetId<Shader>>;
}

pub struct MaterialLayout<M: Material> {
    layout: BindGroupLayout,
    _marker: std::marker::PhantomData<M>,
}

impl<M: Material> Clone for MaterialLayout<M> {
    fn clone(&self) -> Self {
        Self {
            layout: self.layout.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M: Material> MaterialLayout<M> {
    pub fn new(device: &RenderDevice) -> Self {
        let layout = M::create_bind_group_layout(device);
        Self {
            layout,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<M: Material> std::ops::Deref for MaterialLayout<M> {
    type Target = BindGroupLayout;

    fn deref(&self) -> &Self::Target {
        &self.layout
    }
}

impl<M: Material> AsRef<BindGroupLayout> for MaterialLayout<M> {
    fn as_ref(&self) -> &BindGroupLayout {
        &self.layout
    }
}

impl<M: Material> Resource for MaterialLayout<M> {}
impl<M: Material> RenderResource for MaterialLayout<M> {
    type Arg = Read<RenderDevice>;

    fn extract(device: ecs::system::ArgItem<Self::Arg>) -> Result<Self, super::ExtractError<()>> {
        Ok(Self::new(&device))
    }
}

pub struct MaterialBinding<M: Material> {
    bind_group: BindGroup,
    _marker: std::marker::PhantomData<M>,
}

impl<M: Material> std::ops::Deref for MaterialBinding<M> {
    type Target = BindGroup;

    fn deref(&self) -> &Self::Target {
        &self.bind_group
    }
}

impl<M: Material> RenderAsset for MaterialBinding<M> {}

impl<M: Material> RenderAssetExtractor for M {
    type RenderAsset = MaterialBinding<M>;

    type Arg = (
        Read<RenderDevice>,
        Option<Read<MaterialLayout<M>>>,
        SCommands,
        M::Arg,
    );

    fn extract(
        asset: Self,
        arg: &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self::RenderAsset, super::ExtractError<Self>> {
        let (device, layout, commands, arg) = arg;
        let layout = match layout.as_ref() {
            Some(layout) => layout,
            None => {
                commands.add(ExtractResource::<MaterialLayout<M>>::new());
                return Err(super::ExtractError::Retry(asset));
            }
        };

        let binding = asset
            .create_bind_group(device, &layout, arg)
            .map_err(|_| super::ExtractError::Retry(asset))?;

        Ok(MaterialBinding {
            bind_group: binding,
            _marker: std::marker::PhantomData,
        })
    }
}

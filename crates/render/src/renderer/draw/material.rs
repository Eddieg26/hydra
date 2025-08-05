use crate::{
    AsBinding, BindGroup, BindGroupLayout, ExtractError, RenderAsset, RenderDevice, RenderResource,
    Shader,
    draw::{
        View,
        model::{ShaderModel, ShaderPhase},
    },
};
use asset::{Asset, AssetId, ErasedId};
use ecs::{Resource, system::Always, unlifetime::Read};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum DepthWrite {
    Auto,
    Enabled,
    Disabled,
}

pub trait Material: Asset + AsBinding + Clone {
    type View: View;

    type Model: ShaderModel;

    type Phase: ShaderPhase<View = Self::View>;

    fn depth_write() -> DepthWrite {
        DepthWrite::Auto
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

    type Condition = Always<true>;

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

    type Arg = (Read<RenderDevice>, Read<MaterialLayout<M>>, M::Arg);

    fn extract(
        _: AssetId<Self::Source>,
        asset: Self::Source,
        (device, layout, arg): &mut ecs::ArgItem<Self::Arg>,
    ) -> Result<Self, ExtractError<Self::Source>> {
        let bind_group = asset
            .create_bind_group(device, &layout, arg)
            .map_err(|_| ExtractError::Retry(asset))?;

        Ok(Self(bind_group, std::marker::PhantomData))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaterialId(ErasedId);
impl std::ops::Deref for MaterialId {
    type Target = ErasedId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<M: Material> From<AssetId<M>> for MaterialId {
    fn from(id: AssetId<M>) -> Self {
        Self(id.into())
    }
}

impl From<ErasedId> for MaterialId {
    fn from(id: ErasedId) -> Self {
        Self(id)
    }
}

impl<M: Material> Into<AssetId<M>> for MaterialId {
    fn into(self) -> AssetId<M> {
        AssetId::from(self.0)
    }
}

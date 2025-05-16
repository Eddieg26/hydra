use ecs::CommandBuffer;

pub struct AssetMeta {
    load: fn(&mut CommandBuffer, Vec<u8>),
}

pub struct AssetRegistry {}

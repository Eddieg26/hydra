use encase::{ShaderType, internal::WriteInto};

pub mod cpu;
pub mod drawable;
pub mod gpu;
pub mod material;
pub mod pass;
pub mod view;

pub trait ShaderData: Copy + ShaderType + WriteInto + Send + Sync + 'static {}

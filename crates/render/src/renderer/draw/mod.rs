use encase::{ShaderType, internal::WriteInto};

pub mod cpu;
pub mod drawable;
pub mod material;
pub mod pass;
pub mod view;

pub use drawable::*;
pub use material::*;
pub use pass::*;
pub use view::*;

pub trait ShaderData: Copy + ShaderType + WriteInto + Send + Sync + 'static {}
impl<S: Copy + ShaderType + WriteInto + Send + Sync + 'static> ShaderData for S {}

use std::borrow::Cow;

pub mod binding;
pub mod buffer;
pub mod extract;
pub mod id;
pub mod mesh;
pub mod pipeline;
pub mod shader;
pub mod texture;
pub mod bindless;

pub use binding::*;
pub use buffer::*;
pub use extract::*;
pub use mesh::*;
pub use pipeline::*;
pub use shader::*;
pub use texture::*;
pub use bindless::*;

pub type Label = Cow<'static, str>;

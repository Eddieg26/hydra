use std::borrow::Cow;

pub mod binding;
pub mod buffer;
pub mod extract;
pub mod id;
pub mod pipeline;
pub mod shader;
pub mod texture;

pub type Label = Cow<'static, str>;

pub use binding::*;
pub use buffer::*;
pub use extract::*;
pub use id::*;
pub use pipeline::*;
pub use shader::*;
pub use texture::*;

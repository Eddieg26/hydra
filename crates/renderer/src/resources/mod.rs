use std::borrow::Cow;

pub mod buffer;
pub mod extract;
pub mod id;
pub mod shader;
pub mod texture;

pub type Label = Cow<'static, str>;

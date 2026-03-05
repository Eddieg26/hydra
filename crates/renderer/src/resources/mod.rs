use std::borrow::Cow;

pub mod buffer;
pub mod id;
pub mod texture;

pub type Label = Cow<'static, str>;

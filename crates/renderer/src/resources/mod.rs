use std::borrow::Cow;

pub mod buffer;
pub mod id;

pub type Label = Cow<'static, str>;

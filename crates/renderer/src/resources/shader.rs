use std::borrow::Cow;
use asset::Asset;

#[derive(Clone, Asset)]
pub struct Shader {
    data: Cow<'static, str>,
}

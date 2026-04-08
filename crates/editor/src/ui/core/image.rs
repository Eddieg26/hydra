use math::{Size};
use slotmap::new_key_type;

new_key_type! {pub struct ImageHandle;}

pub trait ImageResolver {
    fn size(&self, image: ImageHandle) -> Size;
}

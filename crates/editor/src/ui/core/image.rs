use math::Vec2;
use slotmap::new_key_type;

new_key_type! {pub struct ImageHandle;}

pub trait ImageResolver {
    fn size(&self, image: ImageHandle) -> Vec2;
}

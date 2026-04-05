use bytemuck::{Pod, Zeroable};
use encase::ShaderType;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable, ShaderType, serde::Serialize, serde::Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn as_slice(&self) -> [f32; 4] {
        [self.r, self.b, self.g, self.a]
    }

    pub fn to_bytes(&self) -> (u8, u8, u8, u8) {
        let r = (self.r * 255.0) as u8;
        let g = (self.g * 255.0) as u8;
        let b = (self.b * 255.0) as u8;
        let a = (self.a * 255.0) as u8;
        (r, g, b, a)
    }

    pub fn pack(&self) -> u32 {
        let r = (self.r * 255.0) as u8;
        let g = (self.g * 255.0) as u8;
        let b = (self.b * 255.0) as u8;
        let a = (self.a * 255.0) as u8;

        u32::from_be_bytes([r, g, b, a])
    }
}

impl From<wgpu::Color> for Color {
    fn from(value: wgpu::Color) -> Self {
        Self {
            r: value.r as f32,
            g: value.g as f32,
            b: value.b as f32,
            a: value.a as f32,
        }
    }
}

impl From<&[f32; 4]> for Color {
    fn from(value: &[f32; 4]) -> Self {
        Self {
            r: value[0],
            g: value[1],
            b: value[2],
            a: value[3],
        }
    }
}

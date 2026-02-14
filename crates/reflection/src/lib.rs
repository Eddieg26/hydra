pub use derive_reflect::Reflect;

#[derive(Clone, Debug)]
pub struct ReflectValue {
    pub name: &'static str,
    pub value: ReflectType,
}

#[derive(Clone, Debug)]
pub struct ReflectObject {
    pub name: &'static str,
    pub fields: Vec<ReflectValue>,
}

#[allow(non_camel_case_types)]
#[derive(Clone, Debug)]
pub enum ReflectType {
    Unit,
    u8(u8),
    u16(u16),
    u32(u32),
    u64(u64),
    u128(u128),
    i8(i8),
    i16(i16),
    i32(i32),
    i64(i64),
    i128(i128),
    String(String),
    Array(Vec<ReflectType>),
    Object(Vec<ReflectValue>),
    Enum {
        value: Box<ReflectObject>,
        variants: Vec<ReflectObject>,
    },
}

impl ReflectType {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::u8(_) => "u8",
            Self::u16(_) => "u16",
            Self::u32(_) => "u32",
            Self::u64(_) => "u64",
            Self::u128(_) => "u128",
            Self::i8(_) => "i8",
            Self::i16(_) => "i16",
            Self::i32(_) => "i32",
            Self::i64(_) => "i64",
            Self::i128(_) => "i128",
            Self::String(_) => "String",
            Self::Array(_) => "Array",
            Self::Object(_) => "Object",
            Self::Enum { .. } => "Enum",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ReflectError {
    NotSupported,
    InvalidValue(ReflectValue),
    InvalidType {
        name: &'static str,
        expected: &'static str,
    },
    UnknownField {
        name: String,
    },
    MissingField {
        name: &'static str,
    },
}

pub trait Reflect: Send + Sync + Clone + Default + 'static {
    fn type_name(&self) -> &'static str;
    fn reflect(&self) -> ReflectType;
    fn set_reflect(&mut self, value: ReflectType) -> Result<(), ReflectError>;
    fn construct(value: ReflectType) -> Self {
        let mut ret = Self::default();
        let _ = ret.set_reflect(value);
        ret
    }
}

macro_rules! impl_reflect_primitive {
    ($($t:ty => $variant:ident),* $(,)?) => {
        $(
            impl Reflect for $t {
                fn type_name(&self) -> &'static str {
                    stringify!($t)
                }

                fn reflect(&self) -> ReflectType {
                    ReflectType::$variant(*self)
                }

                fn set_reflect(&mut self, value: ReflectType) -> Result<(), ReflectError> {
                    match value {
                        ReflectType::$variant(v) => {
                            *self = v;
                            Ok(())
                        }
                        v => Err(ReflectError::InvalidType {
                            name: v.type_name(),
                            expected: stringify!($t),
                        }),
                    }
                }
            }
        )*
    };
}

impl_reflect_primitive! {
    u8 => u8,
    u16 => u16,
    u32 => u32,
    u64 => u64,
    u128 => u128,
    i8 => i8,
    i16 => i16,
    i32 => i32,
    i64 => i64,
    i128 => i128,
}

impl<R: Reflect> Reflect for Vec<R> {
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Vec<R>>()
    }

    fn reflect(&self) -> ReflectType {
        let values = self.iter().map(|v| v.reflect());
        ReflectType::Array(values.collect())
    }

    fn set_reflect(&mut self, value: ReflectType) -> Result<(), ReflectError> {
        match value {
            ReflectType::Array(mut types) => {
                let values = types.drain(..).map(|ty| R::construct(ty));
                *self = values.collect();
                Ok(())
            }
            _ => Err(ReflectError::InvalidType {
                name: value.type_name(),
                expected: self.type_name(),
            }),
        }
    }
}

impl Reflect for String {
    fn type_name(&self) -> &'static str {
        "String"
    }

    fn reflect(&self) -> ReflectType {
        ReflectType::String(self.clone())
    }

    fn set_reflect(&mut self, value: ReflectType) -> Result<(), ReflectError> {
        match value {
            ReflectType::String(v) => {
                *self = v;
                Ok(())
            }
            v => Err(ReflectError::InvalidType {
                name: v.type_name(),
                expected: "String",
            }),
        }
    }
}

impl Reflect for () {
    fn type_name(&self) -> &'static str {
        "unit"
    }

    fn reflect(&self) -> ReflectType {
        ReflectType::Unit
    }

    fn set_reflect(&mut self, _: ReflectType) -> Result<(), ReflectError> {
        Ok(())
    }
}

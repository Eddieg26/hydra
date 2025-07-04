use std::error::Error;

pub trait GraphResource: Sized + Send + Sync + 'static {
    type Desc: Send + Sync + 'static;

    type Error: Error + 'static;

    fn create(desc: &Self::Desc) -> Result<Self, Self::Error>;
}

pub struct GraphResourceId<R: GraphResource>(usize, std::marker::PhantomData<R>);

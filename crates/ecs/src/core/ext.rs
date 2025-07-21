pub fn short_type_name<T: ?Sized>() -> &'static str {
    let name = std::any::type_name::<T>();
    name.split("::").last().unwrap_or(name)
}

pub fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

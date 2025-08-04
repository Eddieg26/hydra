pub fn short_type_name<T: ?Sized>() -> &'static str {
    let name = std::any::type_name::<T>();
    let name = name.split("::").last().unwrap_or(name);

    let start = name.find("<").unwrap_or(0);
    let end = name.find(">").unwrap_or(name.len());

    &name[start..end]
}

pub fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

pub fn short_type_name<T: ?Sized>() -> &'static str {
    let name = std::any::type_name::<T>();
    match name.rfind(":") {
        Some(index) => &name[index + 1..],
        None => name,
    }
}

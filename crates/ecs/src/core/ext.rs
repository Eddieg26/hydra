pub fn short_type_name<T: ?Sized>() -> &'static str {
    let name = std::any::type_name::<T>();
    let bytes = name.as_bytes();
    let mut i = bytes.len();

    // Walk backwards looking for the last occurrence of "::"
    while i >= 2 {
        if bytes[i - 2] == b':' && bytes[i - 1] == b':' {
            return &name[i..];
        }
        i -= 1;
    }

    name
}

pub fn align_to(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

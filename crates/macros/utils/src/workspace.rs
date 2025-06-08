use crate::Symbol;
use std::path::PathBuf;
use syn::Ident;

pub const MANIFEST: Symbol = Symbol::new("CARGO_MANIFEST_DIR");
pub const CRATE_NAME: Symbol = Symbol::new("CARGO_PKG_NAME");

pub fn get_manifest_dir() -> Option<PathBuf> {
    std::env::var_os(MANIFEST).map(PathBuf::from)
}

pub fn get_crate_name() -> Option<String> {
    std::env::var(CRATE_NAME).ok()
}

pub fn get_crate_path(name: &'static str) -> Ident {
    let current_name = match get_crate_name() {
        Some(name) => name,
        None => return Ident::new(name, proc_macro2::Span::call_site()),
    };

    if current_name.as_str() == name {
        return Ident::new("crate", proc_macro2::Span::call_site());
    } else if let Some(index) = current_name.find(name) {
        if index == 0 && current_name.get(index..index + 2) == Some("::") {
            let mut name = current_name.trim_start_matches(name).to_string();
            if name.starts_with("::") {
                name = name.trim_start_matches("::").to_string();
            }
            name.insert_str(0, "crate");
            return Ident::new(&name, proc_macro2::Span::call_site());
        }
    }

    return Ident::new(name, proc_macro2::Span::call_site());
}

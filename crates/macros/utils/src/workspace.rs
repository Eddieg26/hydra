use crate::Symbol;
use std::path::PathBuf;

pub const MANIFEST: Symbol = Symbol::new("CARGO_MANIFEST_DIR");
pub const CRATE_NAME: Symbol = Symbol::new("CARGO_PKG_NAME");

pub fn get_manifest_dir() -> Option<PathBuf> {
    std::env::var_os(MANIFEST).map(PathBuf::from)
}

pub fn get_crate_name() -> Option<String> {
    std::env::var(CRATE_NAME).ok()
}

pub fn get_crate_path(name: &'static str) -> Option<String> {
    let current_name = match get_crate_name() {
        Some(name) => name,
        None => return Some(name.to_string()),
    };

    if current_name.as_str() == name {
        return Some("crate::".to_string());
    } else if current_name.starts_with(name) {
        let mut name = current_name.trim_start_matches(name).to_string();
        if name.starts_with("::") {
            name = name.trim_start_matches("::").to_string();
        }
        name.insert_str(0, "crate::");
        return Some(name);
    } else {
        let mut name = name.to_string();
        name.push_str("::");
        return Some(name);
    }
}

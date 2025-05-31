use quote::ToTokens;
use std::{ffi::OsStr, fmt::Display};
use syn::Ident;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Symbol(&'static str);

impl Symbol {
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl std::ops::Deref for Symbol {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl PartialEq<Symbol> for &Ident {
    fn eq(&self, word: &Symbol) -> bool {
        *self == word.0
    }
}

impl PartialEq<Symbol> for syn::Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl PartialEq<Symbol> for &syn::Path {
    fn eq(&self, word: &Symbol) -> bool {
        self.is_ident(word.0)
    }
}

impl Display for Symbol {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}

impl AsRef<str> for Symbol {
    fn as_ref(&self) -> &str {
        self.0
    }
}

impl AsRef<OsStr> for Symbol {
    fn as_ref(&self) -> &OsStr {
        OsStr::new(self.0)
    }
}

impl ToTokens for Symbol {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = self.0;
        let ident = syn::Ident::new(name, proc_macro2::Span::call_site());
        tokens.extend(quote::quote! { #ident });
    }
}

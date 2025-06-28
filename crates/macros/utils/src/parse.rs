use std::{fmt::Display, str::FromStr};
use syn::{Error, Result};

pub trait ParseExp {
    fn parse_base10<N>(&self) -> Result<N>
    where
        N: FromStr,
        N::Err: Display;
    fn parse_str(&self) -> Result<String>;
    fn parse_bool(&self) -> Result<bool>;
}

impl ParseExp for syn::Expr {
    fn parse_base10<N>(&self) -> Result<N>
    where
        N: FromStr,
        N::Err: Display,
    {
        let syn::Expr::Lit(exp) = self else {
            return Err(Error::new_spanned(self, ""));
        };

        match &exp.lit {
            syn::Lit::Int(value) => value.base10_parse::<N>(),
            _ => return Err(Error::new_spanned(self, "")),
        }
    }

    fn parse_str(&self) -> Result<String> {
        let syn::Expr::Lit(exp) = self else {
            return Err(Error::new_spanned(self, ""));
        };

        let syn::Lit::Str(str) = &exp.lit else {
            return Err(Error::new_spanned(self, ""));
        };

        Ok(str.value())
    }

    fn parse_bool(&self) -> Result<bool> {
        let syn::Expr::Lit(exp) = self else {
            return Err(Error::new_spanned(self, ""));
        };

        let syn::Lit::Bool(bool) = &exp.lit else {
            return Err(Error::new_spanned(self, ""));
        };

        Ok(bool.value())
    }
}

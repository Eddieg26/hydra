use quote::ToTokens;
use std::fmt::Display;

#[derive(Default)]
pub struct Context {
    errors: Option<Vec<syn::Error>>,
}

impl Context {
    /// Add an error to the context object.
    pub fn add_error(&mut self, err: syn::Error) {
        self.errors.as_mut().unwrap().push(err);
    }

    /// Add an error to the context object, using a spanned object and a message.
    ///
    /// This is a convenience method that converts the object into a token stream for spanning in error messages.
    pub fn add_spanned_error<A: ToTokens, T: Display>(&mut self, obj: A, msg: T) {
        self.errors
            .as_mut()
            .unwrap()
            // Curb monomorphization from generating too many identical methods.
            .push(syn::Error::new_spanned(obj.into_token_stream(), msg));
    }

    /// Consume this object, producing a formatted error string if there are errors.
    pub fn check(mut self) -> syn::Result<()> {
        let mut errors = self.errors.take().unwrap().into_iter();

        let mut combined = match errors.next() {
            Some(first) => first,
            None => return Ok(()),
        };

        for rest in errors {
            combined.combine(rest);
        }

        Err(combined)
    }
}

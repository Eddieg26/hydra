use macro_utils::{
    quote::{format_ident, quote},
    syn::{Ident, LitInt, parse::Parse, parse_macro_input, token::Comma},
};
use proc_macro::TokenStream;

struct Info {
    ident: Ident,
    impl_ident: Ident,
    start: usize,
    end: usize,
}

impl Parse for Info {
    fn parse(input: macro_utils::syn::parse::ParseStream) -> macro_utils::syn::Result<Self> {
        let impl_ident = input.parse::<Ident>()?;
        input.parse::<Comma>()?;

        let ident = input.parse::<Ident>()?;
        input.parse::<Comma>()?;

        let start = input.parse::<LitInt>()?.base10_parse()?;

        let (mut start, mut end) = match input
            .parse::<Comma>()
            .and_then(|_| input.parse::<LitInt>())
            .and_then(|i| i.base10_parse())
        {
            Ok(end) => (start, end),
            Err(_) => (0, start),
        };

        if start > end {
            std::mem::swap(&mut start, &mut end);
        }

        Ok(Info {
            ident,
            impl_ident,
            start,
            end,
        })
    }
}

#[proc_macro]
pub fn variable_impl(input: TokenStream) -> TokenStream {
    let info: Info = parse_macro_input!(input);
    let impl_ident = info.impl_ident;
    let idents = (0..=info.end)
        .map(|index| {
            let ident = format_ident!("{}{}", info.ident, index);
            quote! {#ident}
        })
        .collect::<Vec<_>>();

    let impls = (info.start.max(1)..info.end).map(|index| {
        let vars = &idents[..index];
        let vars = quote! {#(#vars),*};
        quote! {#impl_ident!(#vars);}
    });

    TokenStream::from(quote! {
        #(
            #impls
        )*
    })
}

use syn::{ext::IdentExt, parse::Parse, Ident, Token};

pub(crate) fn field_name(field: &syn::Field) -> String {
    field
        .ident
        .as_ref()
        .expect("Must be a named field")
        .to_string()
}

pub(crate) enum DefaultAssignment {
    DefaultCall,
    Expression(syn::Expr),
    FunctionCall(syn::Path),
}

impl Parse for DefaultAssignment {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        Ok(if input.peek(Ident::peek_any) && input.peek2(Token![=]) {
            let _ = input.parse::<syn::Ident>()?;
            let _ = input.parse::<Token![=]>()?;
            DefaultAssignment::FunctionCall(input.parse()?)
        } else {
            DefaultAssignment::Expression(input.parse()?)
        })
    }
}

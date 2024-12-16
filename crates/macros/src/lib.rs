mod config_property;
mod general;
mod generic_builder;

use proc_macro::TokenStream;

macro_rules! propagate_err {
    ($expr:expr) => {
        match $expr {
            Ok(data) => data,
            Err(err) => return err.to_compile_error().into(),
        }
    };
}

pub(crate) use propagate_err;

#[proc_macro_derive(ConfigProperty, attributes(cfg_prop))]
pub fn config_property(item: TokenStream) -> TokenStream {
    config_property::make_derive(item)
}

#[proc_macro_derive(GenericBuilder, attributes(gbuilder))]
pub fn generic_builder(item: TokenStream) -> TokenStream {
    generic_builder::make_derive(item)
}

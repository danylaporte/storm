use proc_macro2::TokenStream;
use syn::{parse::Parse, Attribute};

pub trait AttrsExt {
    fn attrs(&self) -> &Vec<Attribute>;

    fn parse_attr<T>(&self, name: &str) -> Result<Option<T>, TokenStream>
    where
        T: Parse,
    {
        Ok(match self.attrs().iter().find(|a| a.path.is_ident(name)) {
            Some(a) => Some(a.parse_args::<T>().map_err(|e| e.to_compile_error())?),
            None => None,
        })
    }
}

impl AttrsExt for Vec<Attribute> {
    fn attrs(&self) -> &Vec<Attribute> {
        self
    }
}

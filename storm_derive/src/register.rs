use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, ItemFn, Path};

pub fn register(attr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_path: Path = match syn::parse::<syn::Meta>(attr) {
        Ok(syn::Meta::NameValue(nv)) if nv.path.is_ident("crate") => match nv.value {
            syn::Expr::Path(p) => p.path,
            _ => {
                return syn::Error::new(nv.value.span(), "expected path")
                    .to_compile_error()
                    .into()
            }
        },
        Ok(_) | Err(_) => syn::parse_quote!(storm), // default / no attr
    };

    let func = parse_macro_input!(item as ItemFn);
    if !func.sig.inputs.is_empty() {
        return syn::Error::new(func.sig.inputs.span(), "function must take no arguments")
            .to_compile_error()
            .into();
    }

    let fn_name = &func.sig.ident;
    let reg_ident = format_ident!("__{}", fn_name.to_string().to_uppercase());

    quote! {
        #func
        #[#crate_path::linkme::distributed_slice(#crate_path::registry::__REGISTRATION)]
        #[linkme(crate = #crate_path::linkme)]
        static #reg_ident: fn() = #fn_name;
    }
    .into()
}

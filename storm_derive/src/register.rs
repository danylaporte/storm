use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Ident, ItemFn, Path, parse_macro_input, spanned::Spanned};

/// Registers `body` to run in the registration phase.
///
/// Emits the `linkme` slice element directly instead of `#[storm::register]` to avoid a nested
/// proc-macro expansion.
pub(crate) fn registration(
    name: &Ident,
    body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        #[allow(non_upper_case_globals)]
        #[storm::linkme::distributed_slice(storm::registry::__REGISTRATION)]
        #[linkme(crate = storm::linkme)]
        static #name: fn() = || { #body };
    }
}

/// Body of a function returning the `storm::CtxVar<#ty>` singleton.
///
/// The var must also be touched in a `registration` so it is allocated before any `Ctx` exists.
pub(crate) fn ctx_var(ty: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! {
        static V: storm::LazyCtxVar<#ty> = storm::LazyCtxVar::new();
        V.get()
    }
}

pub fn register(attr: TokenStream, item: TokenStream) -> TokenStream {
    let crate_path: Path = match syn::parse::<syn::Meta>(attr) {
        Ok(syn::Meta::NameValue(nv)) if nv.path.is_ident("crate") => match nv.value {
            syn::Expr::Path(p) => p.path,
            _ => {
                return syn::Error::new(nv.value.span(), "expected path")
                    .to_compile_error()
                    .into();
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

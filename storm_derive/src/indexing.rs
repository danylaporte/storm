use crate::TokenStreamExt;
use inflector::Inflector;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Error, FnArg, Ident, Item, ItemFn, ReturnType, Type};

#[allow(unused_variables)]
pub(crate) fn indexing(item: Item) -> TokenStream {
    match &item {
        Item::Fn(f) => indexing_fn(f),
        _ => return Error::new(item.span(), "Only function is supported.").to_compile_error(),
    }
}

fn indexing_fn(f: &ItemFn) -> TokenStream {
    let vis = &f.vis;
    let name = &f.sig.ident;
    let index_name = Ident::new(&name.to_string().to_pascal_case(), name.span());

    let ty = match &f.sig.output {
        ReturnType::Type(_, t) => t,
        ReturnType::Default => {
            return Error::new(f.sig.output.span(), "Index must have a return value.")
                .to_compile_error()
        }
    };

    let mut args = Vec::new();

    for arg in &f.sig.inputs {
        match arg {
            FnArg::Typed(t) => args.push(t),
            FnArg::Receiver(r) => {
                return Error::new(r.span(), "Self is not expected here.").to_compile_error()
            }
        };
    }

    let as_ref_wheres = args
        .iter()
        .map(|a| unref(&a.ty))
        .map(|t| quote!(AsRef<#t>))
        .fold(None, |acc, v| {
            Some(match acc {
                Some(acc) => quote!(#acc + #v),
                None => quote!(where C: #v),
            })
        });

    let as_refs = args.iter().map(|_| quote!(ctx.as_ref(),)).ts();

    let init_version = args
        .iter()
        .map(|t| unref(&t.ty))
        .map(|t| quote!(storm::GetVersion::get_version(AsRef::<#t>::as_ref(ctx)).unwrap_or(0)))
        .fold(None, |acc, v| {
            Some(match acc {
                Some(acc) => quote!(std::cmp::max(#acc, #v)),
                None => v,
            })
        });

    quote! {
        #vis struct #index_name(#ty, u64);

        impl std::ops::Deref for #index_name {
            type Target = #ty;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<C> storm::GetOrLoadSync<#index_name, C> for storm::OnceCell<#index_name> #as_ref_wheres,
        {
            fn get_or_load_sync(&self, ctx: &C) -> &#index_name {
                self.get_or_init(|| #index_name(#name(#as_refs), #init_version))
            }

            fn get_mut(&mut self) -> Option<&mut #index_name> {
                self.get_mut()
            }
        }

        impl storm::GetVersion for #index_name {
            fn get_version(&self) -> Option<u64> {
                Some(self.1)
            }
        }

        #f
    }
}

fn unref(t: &Type) -> &Type {
    match t {
        Type::Reference(r) => unref(&*r.elem),
        _ => t,
    }
}

use super::{
    builders::{DeleteBuilder, ParamsBuilder},
    TypeAttrs,
};
use darling::util::SpannedValue;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use std::marker::PhantomData;
use syn::LitInt;

pub(super) struct Delete<'a, S> {
    attrs: &'a TypeAttrs,
    _s: PhantomData<S>,
}

impl<'a, S> Delete<'a, S> {
    pub fn new(attrs: &'a TypeAttrs) -> Self {
        Self {
            attrs,
            _s: PhantomData,
        }
    }
}

impl<'a, S> ToTokens for Delete<'a, S>
where
    S: AttrsSelector,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let table = S::table(self.attrs);

        if table.is_empty() {
            return;
        }

        let mut errors = Vec::new();
        let mut params = ParamsBuilder::default();
        let mut delete = DeleteBuilder::default();

        let keys = S::keys(self.attrs, &mut errors);

        add_keys(&keys, &mut params, &mut delete);

        let sql = delete.to_sql_lit(table);

        tokens.append_all(quote! {
            storm::tri!(storm_mssql::Execute::execute(provider, #sql, #params).await);
            #(#errors)*
        });
    }
}

#[cold]
fn add_key_many(keys: &[&str], params: &mut ParamsBuilder, builder: &mut DeleteBuilder) {
    for (index, column) in keys.iter().enumerate() {
        let i = LitInt::new(&index.to_string(), Span::call_site());
        add_key_single(column, quote!(&k.#i as _), params, builder);
    }
}

fn add_key_single(
    column: &str,
    ts: TokenStream,
    params: &mut ParamsBuilder,
    builder: &mut DeleteBuilder,
) {
    let i = params.add_ts(ts);
    builder.add_key(column, &i.to_string());
}

fn add_keys(keys: &[&str], params: &mut ParamsBuilder, builder: &mut DeleteBuilder) {
    match keys {
        [k] => add_key_single(k, quote!(k as _), params, builder),
        _ => add_key_many(keys, params, builder),
    }
}

pub(super) trait AttrsSelector {
    fn keys<'a>(attrs: &'a TypeAttrs, errors: &mut Vec<TokenStream>) -> Vec<&'a str>;
    fn table(attrs: &TypeAttrs) -> &SpannedValue<String>;
}

macro_rules! selector {
    ($t:ty, $keys:ident, $table:ident) => {
        impl AttrsSelector for $t {
            fn keys<'a>(attrs: &'a TypeAttrs, errors: &mut Vec<TokenStream>) -> Vec<&'a str> {
                attrs.$keys(errors)
            }

            fn table(attrs: &TypeAttrs) -> &SpannedValue<String> {
                &attrs.$table
            }
        }
    };
}

pub(super) mod selectors {
    use super::*;

    pub struct Normal;
    pub struct Translate;

    selector!(Normal, keys, table);
    selector!(Translate, translate_keys, translate_table);
}

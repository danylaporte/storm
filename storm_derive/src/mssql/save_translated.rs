use super::{
    attrs::{check_empty, check_required},
    builders::{ParamsBuilder, UpsertBuilder},
    TypeAttrs,
};
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt as _};
use syn::{Field, LitInt};

pub(super) struct SaveTranslated<'a> {
    attrs: &'a TypeAttrs,
    params: ParamsBuilder,
    upsert: UpsertBuilder,
}

impl<'a> SaveTranslated<'a> {
    pub fn new(attrs: &'a TypeAttrs) -> Self {
        Self {
            attrs,
            params: Default::default(),
            upsert: Default::default(),
        }
    }

    pub fn add_field(&mut self, field: &Field, column: &str) {
        let ident = &field.ident;
        let param_index = self.params.add_ts(quote!(&&self.#ident[culture] as _));

        self.upsert.add_field(column, &param_index.to_string());
    }
}

impl<'a> ToTokens for SaveTranslated<'a> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut errors = Vec::new();

        if self.params.is_empty() {
            check_empty(&self.attrs.translate_table, &mut errors);
            check_empty(&self.attrs.translate_keys, &mut errors);
        } else {
            check_required(&self.attrs.translate_table, &mut errors);

            let mut params = self.params.clone();
            let mut upsert = self.upsert.clone();
            let keys = self.attrs.translate_keys(&mut errors);

            add_keys(&keys, &mut params, &mut upsert);

            upsert.add_key("culture", &params.add_ts(quote!(&culture as _)).to_string());

            let sql = upsert.to_sql_lit(&self.attrs.translate_table);

            tokens.append_all(quote! {
                for &culture in Culture::DB_CULTURES.iter() {
                    storm_mssql::Execute::execute(provider, #sql, #params).await?;
                }
            });
        }

        tokens.append_all(quote!(#(#errors)*));
    }
}

#[cold]
fn add_key_many(keys: &[&str], params: &mut ParamsBuilder, builder: &mut UpsertBuilder) {
    for (index, column) in keys.iter().enumerate() {
        let i = LitInt::new(&index.to_string(), Span::call_site());
        add_key_single(column, quote!(&k.#i as _), params, builder);
    }
}

fn add_key_single(
    column: &str,
    ts: TokenStream,
    params: &mut ParamsBuilder,
    builder: &mut UpsertBuilder,
) {
    let i = params.add_ts(ts);
    builder.add_key(column, &i.to_string());
}

fn add_keys(keys: &[&str], params: &mut ParamsBuilder, builder: &mut UpsertBuilder) {
    if keys.len() == 1 {
        add_key_single(keys[0], quote!(&k as _), params, builder);
    } else {
        add_key_many(keys, params, builder);
    }
}

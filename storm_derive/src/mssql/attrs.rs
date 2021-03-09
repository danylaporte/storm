use crate::rename_all::RenameAll;
use darling::{util::SpannedValue, FromDeriveInput, FromField};
use proc_macro2::TokenStream;
use syn::{Error, Ident};

#[derive(Debug, FromField)]
#[darling(attributes(storm))]
pub(super) struct FieldAttrs {
    #[darling(default)]
    pub column: Option<String>,

    #[darling(default)]
    pub load_with: SpannedValue<Option<Ident>>,

    #[darling(default)]
    pub part: bool,

    #[darling(default)]
    pub save_with: SpannedValue<Option<Ident>>,

    #[darling(default)]
    skip: SpannedValue<Option<bool>>,

    #[darling(default)]
    skip_load: SpannedValue<Option<bool>>,

    #[darling(default)]
    skip_save: SpannedValue<Option<bool>>,
}

impl FieldAttrs {
    pub fn skip_load(&self) -> bool {
        self.skip_load.unwrap_or_default() || self.skip.unwrap_or_default()
    }

    pub fn skip_save(&self) -> bool {
        self.skip_save.unwrap_or_default() || self.skip.unwrap_or_default()
    }

    pub fn validate_load(&self, errors: &mut Vec<TokenStream>) {
        if let (Some(true), Some(false)) = (*self.skip, *self.skip_load) {
            errors.push(Error::new(self.skip_load.span(), SKIP_IS_INCOMPATIBLE).to_compile_error());
        }
    }

    pub fn validate_save(&self, errors: &mut Vec<TokenStream>) {
        if let (Some(true), Some(false)) = (*self.skip, *self.skip_save) {
            errors.push(Error::new(self.skip_save.span(), SKIP_IS_INCOMPATIBLE).to_compile_error());
        }

        if self.skip_save() && self.save_with.is_some() {
            errors.push(Error::new(self.save_with.span(), "Save is skipped.").to_compile_error());
        }

        if self.part && self.save_with.is_some() {
            errors.push(
                Error::new(self.save_with.span(), "Ignored on part field.").to_compile_error(),
            );
        }
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(storm))]
pub(super) struct TypeAttrs {
    pub table: SpannedValue<String>,
    pub keys: SpannedValue<String>,

    #[darling(default)]
    pub rename_all: Option<RenameAll>,

    #[darling(default)]
    pub translate_table: SpannedValue<String>,

    #[darling(default)]
    pub translate_keys: SpannedValue<String>,
}

impl TypeAttrs {
    pub fn keys(&self, errors: &mut Vec<TokenStream>) -> Vec<&str> {
        let vec = self.keys_internal();

        if vec.is_empty() {
            errors.push(
                Error::new(self.keys.span(), "Must specify at least one key.").to_compile_error(),
            );
        }

        vec
    }

    pub fn keys_internal(&self) -> Vec<&str> {
        self.keys.split(',').filter(|s| !s.is_empty()).collect()
    }

    pub fn translate_keys(&self, errors: &mut Vec<TokenStream>) -> Vec<&str> {
        if self.translate_table.is_empty() {
            return Vec::new();
        }

        let keys = self.keys_internal();

        if self.translate_keys.is_empty() {
            return keys;
        }

        let translate_keys = self.translate_keys_internal();

        if translate_keys.len() != keys.len() {
            errors.push(
                Error::new(
                    self.translate_keys.span(),
                    "translate_keys must have the same keys count.",
                )
                .to_compile_error(),
            );
        }

        translate_keys
    }

    fn translate_keys_internal(&self) -> Vec<&str> {
        self.translate_keys
            .split(',')
            .filter(|s| !s.is_empty())
            .collect()
    }
}

const SKIP_IS_INCOMPATIBLE: &str = "`skip` is incompatible.";

pub(super) fn check_empty<'a, T: IsEmpty>(
    v: &'a SpannedValue<T>,
    errors: &mut Vec<TokenStream>,
) -> &'a SpannedValue<T> {
    if !v.is_empty() {
        errors.push(Error::new(v.span(), "Expected to be empty.").to_compile_error());
    }

    v
}

pub(super) fn check_required<'a, T: IsEmpty>(
    v: &'a SpannedValue<T>,
    errors: &mut Vec<TokenStream>,
) -> &'a SpannedValue<T> {
    if v.is_empty() {
        errors.push(Error::new(v.span(), "Expected a value.").to_compile_error());
    }

    v
}

pub(super) trait IsEmpty {
    fn is_empty(&self) -> bool;
}

impl<T: IsEmpty> IsEmpty for &T
where
    T: IsEmpty,
{
    fn is_empty(&self) -> bool {
        (**self).is_empty()
    }
}

impl<T> IsEmpty for Option<T> {
    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

impl IsEmpty for str {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl IsEmpty for String {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl<T> IsEmpty for Vec<T> {
    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

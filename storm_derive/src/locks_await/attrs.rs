use darling::FromDeriveInput;
use syn::Path;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(storm))]
pub(super) struct TypeAttrs {
    pub ctx: Path,
}

use crate::{
    derive_input_ext::DeriveInputExt,
    register::{ctx_var, registration},
};
use darling::{
    FromDeriveInput, FromField, FromMeta,
    util::{Override, SpannedValue},
};
use inflector::Inflector;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    DeriveInput, Error, Expr, GenericArgument, Ident, LitStr, Path, PathArguments, Type,
    Visibility, spanned::Spanned,
};

pub fn generate(input: &DeriveInput) -> TokenStream {
    let implement = try_ts!(implement(input));

    quote! {
        #implement
    }
}

fn implement(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let vis = &input.vis;
    let entity = &input.ident;
    let args = TypeArgs::from_derive_input(input).map_err(|e| e.write_errors())?;

    let entity_name = entity.to_string();
    let table_name = entity_name.to_plural();
    let table_name_lit = LitStr::new(&entity_name, entity.span());
    let table_alias = Ident::new(&table_name, entity.span());

    let init_tbl_fn = Ident::new(
        &format!("__ctx_init_{}", entity_name.to_snake_case()),
        entity.span(),
    );

    let coll_ty = args.collection.ty(entity);
    let (gc, gc_collect) = gc(input)?;
    let (constraints, constraints_init) = constraints(input, &args)?;
    let (trim_fields, trim_error) = match trim(input) {
        Ok(t) => (t, TokenStream::new()),
        Err(e) => (TokenStream::new(), e),
    };
    let tbl_var = ctx_var(quote!(#table_alias));

    // A single registration per entity, for the table and all its constraints.
    let register = registration(
        &init_tbl_fn,
        quote! {
            storm::__register_apply(
                #table_alias::__apply_log,
                storm::ApplyOrder::Table,
                &[<#entity as storm::EntityAccessor>::tbl_var().var_id()],
            );
            #gc_collect
            #constraints_init
        },
    );

    Ok(quote! {
        #vis type #table_alias = #coll_ty;

        impl storm::EntityAccessor for #entity {
            type Tbl = #table_alias;

            #[inline]
            fn applied() -> &'static storm::AppliedEvent<Self> {
                static E: storm::AppliedEvent<#entity> = storm::AppliedEvent::new();
                &E
            }

            #[inline]
            fn cleared() -> &'static storm::ClearEvent {
                static E: storm::ClearEvent = storm::ClearEvent::new();
                &E
            }

            #[inline]
            fn removed() -> &'static storm::RemovedEvent<Self> {
                static E: storm::RemovedEvent<#entity> = storm::RemovedEvent::new();
                &E
            }

            #[inline]
            fn removing() -> &'static storm::RemovingEvent<Self> {
                static E: storm::RemovingEvent<#entity> = storm::RemovingEvent::new();
                &E
            }

            #[inline]
            fn tbl_var() -> storm::CtxVar<Self::Tbl> {
                #tbl_var
            }

            #[inline]
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }

            #[inline]
            fn upserted() -> &'static storm::UpsertedEvent<Self> {
                static E: storm::UpsertedEvent<#entity> = storm::UpsertedEvent::new();
                &E
            }

            #[inline]
            fn upserting() -> &'static storm::UpsertingEvent<Self> {
                static E: storm::UpsertingEvent<#entity> = storm::UpsertingEvent::new();
                &E
            }

            #[inline]
            fn validating() -> &'static storm::ValidatingEvent<Self> {
                static E: storm::ValidatingEvent<#entity> = storm::ValidatingEvent::new();
                &E
            }

            #trim_fields
        }

        #trim_error

        impl storm::CtxTypeInfo for #entity {
            const NAME: &'static str = #table_name_lit;
        }

        #register

        #gc

        #constraints
    })
}

fn gc(input: &DeriveInput) -> Result<(TokenStream, TokenStream), TokenStream> {
    let fields = input.fields()?;
    let ident = &input.ident;
    let types = fields.iter().map(|f| &f.ty);
    let fields = fields.iter().map(|f| &f.ident);

    Ok((
        quote! {
            impl storm::Gc for #ident {
                const SUPPORT_GC: bool = #(<#types as storm::Gc>::SUPPORT_GC ||)* false;

                fn gc(&mut self) {
                    #(storm::Gc::gc(&mut self.#fields);)*
                }
            }
        },
        quote! {
            if <#ident as storm::Gc>::SUPPORT_GC {
                storm::Ctx::on_gc_collect(<#ident as storm::EntityAccessor>::tbl_gc);
            }
        },
    ))
}

#[derive(Clone, Copy, Debug, Default, Eq, FromMeta, PartialEq)]
enum Collection {
    HashTable,

    #[default]
    VecTable,
}

impl Collection {
    fn ty(&self, entity: &Ident) -> TokenStream {
        match self {
            Self::HashTable => quote!(storm::HashTable<#entity>),
            Self::VecTable => quote!(storm::VecTable<#entity>),
        }
    }
}

use type_args::TypeArgs;

// darling generates `field: field` for `#[darling(multiple)]` fields.
#[allow(clippy::redundant_field_names)]
mod type_args {
    use super::{Collection, FromDeriveInput, KeyForeignKeyArgs, Path, UniqueTypeArgs};

    #[derive(Debug, FromDeriveInput)]
    #[darling(attributes(storm), allow_unknown_fields)]
    pub(super) struct TypeArgs {
        #[darling(default)]
        pub collection: Collection,

        /// Foreign keys on key columns.
        #[darling(multiple)]
        pub foreign_key: Vec<KeyForeignKeyArgs>,

        /// Markers of foreign keys declared with `table = ...` in other crates, referencing
        /// this entity.
        #[darling(multiple)]
        pub referenced_by: Vec<Path>,

        /// Unique indexes on one or multiple fields.
        #[darling(multiple)]
        pub unique: Vec<UniqueTypeArgs>,
    }
}

#[derive(Debug, FromField)]
#[darling(attributes(storm), allow_unknown_fields)]
struct FieldArgs {
    ident: Option<Ident>,
    ty: Type,

    #[darling(default)]
    foreign_key: Option<ForeignKeyArgs>,

    /// Trims the string value when the entity is upserted, and the keys of its unique indexes.
    #[darling(default)]
    trim: bool,

    #[darling(default)]
    unique: Option<Override<UniqueFieldArgs>>,
}

#[derive(Debug, Default, FromMeta)]
struct UniqueFieldArgs {
    #[darling(default)]
    name: Option<SpannedValue<String>>,

    /// Type name of the index, instead of `{Entity}{Name}UniqueIndex`.
    #[darling(default)]
    index: Option<SpannedValue<String>>,

    #[darling(default)]
    collation: Collation,

    #[darling(default)]
    filter: Option<Expr>,

    /// No index: the duplicates are found with a table scan. Required for an entity key not
    /// based on `u32` (ex: composite key).
    #[darling(default)]
    no_index: bool,
}

#[derive(Debug, FromMeta)]
struct UniqueTypeArgs {
    /// Comma separated list of fields.
    fields: SpannedValue<String>,

    #[darling(default)]
    name: Option<SpannedValue<String>>,

    /// Type name of the index, instead of `{Entity}{Name}UniqueIndex`.
    #[darling(default)]
    index: Option<SpannedValue<String>>,

    #[darling(default)]
    collation: Collation,

    #[darling(default)]
    filter: Option<Expr>,

    /// See [UniqueFieldArgs::no_index].
    #[darling(default)]
    no_index: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, FromMeta, PartialEq)]
enum Collation {
    Exact,

    #[default]
    Ci,

    Ai,

    AiCi,
}

#[derive(Debug, FromMeta)]
struct ForeignKeyArgs {
    /// The referenced (parent) entity.
    #[darling(default)]
    entity: Option<Path>,

    /// The table of the referenced entity, instead of `entity`, when the parent is in a crate
    /// depending on this one. The parent enforces it with `#[storm(referenced_by = ...)]`.
    #[darling(default)]
    table: Option<SpannedValue<String>>,

    #[darling(default)]
    on_delete: OnDelete,

    /// Type name of the reverse index, instead of `{Entity}{Field}FkIndex`.
    #[darling(default)]
    index: Option<SpannedValue<String>>,

    /// Same as `index`, kept for compatibility (the reverse index is a `FlatSetIndex` by default).
    #[darling(default)]
    flat_index: Option<SpannedValue<String>>,

    /// Uses a `HashFlatSetIndex` as reverse index (slower), for parent keys not based on `u32`.
    #[darling(default)]
    hash: bool,

    /// No reverse index: the children are found with a table scan. Required for a child key
    /// not based on `u32` (ex: composite key).
    #[darling(default)]
    no_index: bool,

    /// Only declared for the schema validation: the database enforces it, nothing is generated.
    /// For entities not saved by storm (ex: no `MssqlSave`).
    #[darling(default)]
    schema_only: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, FromMeta, PartialEq)]
enum OnDelete {
    Cascade,

    #[default]
    Restrict,

    SetNull,
}

/// Foreign key on a column of the key, declared on the type.
#[derive(Debug, FromMeta)]
struct KeyForeignKeyArgs {
    /// The key column referencing the parent (any column with `schema_only`).
    column: SpannedValue<String>,

    /// The referenced (parent) entity.
    #[darling(default)]
    entity: Option<Path>,

    /// The referenced table, `schema_only` only.
    #[darling(default)]
    table: Option<String>,

    #[darling(default)]
    on_delete: OnDelete,

    /// Position of the column in a composite (tuple) key, the whole key when omitted.
    #[darling(default)]
    key: Option<usize>,

    /// Declared for the schema validation only (ex: a column not mapped to a field).
    #[darling(default)]
    schema_only: bool,
}

impl OnDelete {
    fn variant(self) -> TokenStream {
        match self {
            Self::Cascade => quote!(Cascade),
            Self::Restrict => quote!(Restrict),
            Self::SetNull => quote!(SetNull),
        }
    }
}

/// Foreign keys and unique indexes, with the statements registering them.
///
/// Errors are emitted inline so the rest of the `Ctx` implementation is still generated
/// and every invalid field gets reported.
fn constraints(
    input: &DeriveInput,
    args: &TypeArgs,
) -> Result<(TokenStream, TokenStream), TokenStream> {
    let mut out = TokenStream::new();
    let mut init = TokenStream::new();
    let fields = input.fields()?;

    let mut push = |r: Result<Constraint, TokenStream>| match r {
        Ok(c) => {
            out.extend(c.items);
            init.extend(c.init);
        }
        Err(e) => out.extend(e),
    };

    for field in fields {
        let args = match FieldArgs::from_field(field) {
            Ok(args) => args,
            Err(e) => {
                push(Err(e.write_errors()));
                continue;
            }
        };

        if let Some(fk) = &args.foreign_key {
            push(match fk.schema_only {
                true => schema_only_foreign_key(&args, fk),
                false => foreign_key(input, &args, fk),
            });
        }

        if let (Some(unique_args), Some(ident)) = (&args.unique, &field.ident) {
            let default = UniqueFieldArgs::default();
            let unique_args = match unique_args {
                Override::Inherit => &default,
                Override::Explicit(a) => a,
            };

            let def = UniqueDef {
                name: unique_args
                    .name
                    .as_ref()
                    .map(|n| (n.as_str().to_owned(), n.span()))
                    .unwrap_or_else(|| (unraw(ident), ident.span())),
                index: unique_args.index.as_ref(),
                fields: vec![(ident, &field.ty)],
                collation: unique_args.collation,
                filter: unique_args.filter.as_ref(),
                trim: args.trim,
                no_index: unique_args.no_index,
            };

            push(unique(input, def));
        }
    }

    for unique_args in &args.unique {
        push(unique_def(fields, unique_args).and_then(|def| unique(input, def)));
    }

    for fk in &args.foreign_key {
        push(key_foreign_key(input, fk));
    }

    let entity = &input.ident;

    for marker in &args.referenced_by {
        init.extend(quote!(<#marker>::register::<#entity>();));
    }

    Ok((out, init))
}

/// `EntityAccessor::trim_fields`, when fields are `#[storm(trim)]`.
fn trim(input: &DeriveInput) -> Result<TokenStream, TokenStream> {
    let mut idents = Vec::new();

    for field in input.fields()? {
        if !FieldArgs::from_field(field).is_ok_and(|a| a.trim) {
            continue;
        }

        match &field.ident {
            Some(ident) => idents.push(ident),
            None => {
                return Err(
                    Error::new(field.ty.span(), "trim is only supported on named fields.")
                        .to_compile_error(),
                );
            }
        }
    }

    if idents.is_empty() {
        return Ok(TokenStream::new());
    }

    Ok(quote! {
        fn trim_fields(&mut self) {
            #(storm::trim::TrimInPlace::trim_in_place(&mut self.#idents);)*
        }
    })
}

/// Expression returning the field in the entity fields enum, as `Option<Box<dyn storm::Fields>>`.
fn field_of(entity: &Ident, ident: &Ident) -> TokenStream {
    let name = LitStr::new(&ident.to_string().to_pascal_case(), ident.span());

    quote! {{
        #[allow(unused_imports)]
        use storm::{__FieldOfEntity as _, __FieldOfNone as _};
        (&storm::__FieldOf::<#entity>::new()).__field(#name)
    }}
}

/// Generated items of a constraint, and the statements registering it.
struct Constraint {
    items: TokenStream,
    init: TokenStream,
}

struct UniqueDef<'a> {
    name: (String, Span),
    index: Option<&'a SpannedValue<String>>,
    fields: Vec<(&'a Ident, &'a Type)>,
    collation: Collation,
    filter: Option<&'a Expr>,
    trim: bool,
    no_index: bool,
}

fn unique_def<'a>(
    fields: &'a syn::Fields,
    args: &'a UniqueTypeArgs,
) -> Result<UniqueDef<'a>, TokenStream> {
    let span = args.fields.span();
    let mut out = Vec::new();
    let mut trim = false;

    for name in args
        .fields
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let Some(field) = fields
            .iter()
            .find(|f| f.ident.as_ref().is_some_and(|i| unraw(i) == name))
        else {
            return Err(Error::new(span, format!("field `{name}` not found.")).to_compile_error());
        };

        let ident = field.ident.as_ref().expect("ident");

        if out.iter().any(|(i, _)| *i == ident) {
            return Err(Error::new(span, format!("field `{name}` is repeated.")).to_compile_error());
        }

        trim |= FieldArgs::from_field(field).is_ok_and(|a| a.trim);
        out.push((ident, &field.ty));
    }

    if out.is_empty() {
        return Err(Error::new(span, "at least one field is required.").to_compile_error());
    }

    let name = match &args.name {
        Some(n) => (n.as_str().to_owned(), n.span()),
        None => (
            out.iter()
                .map(|(i, _)| unraw(i))
                .collect::<Vec<_>>()
                .join("_"),
            span,
        ),
    };

    Ok(UniqueDef {
        name,
        index: args.index.as_ref(),
        fields: out,
        collation: args.collation,
        filter: args.filter.as_ref(),
        trim,
        no_index: args.no_index,
    })
}

fn unique(input: &DeriveInput, def: UniqueDef) -> Result<Constraint, TokenStream> {
    let vis = &input.vis;
    let entity = &input.ident;
    let (name, span) = def.name;

    let prefix = format!("{entity}{}Unique", name.to_pascal_case());

    let marker = syn::parse_str::<Ident>(&prefix)
        .map(|i| Ident::new(&i.to_string(), span))
        .map_err(|_| {
            Error::new(span, format!("`{name}` is not a valid unique index name."))
                .to_compile_error()
        })?;

    let index = index_ident(def.index, || Ident::new(&format!("{prefix}Index"), span))?;

    let adapt = Ident::new(&format!("{prefix}IndexAdapt"), span);
    let name_lit = LitStr::new(&name, span);

    let field_lits = def
        .fields
        .iter()
        .map(|(i, _)| LitStr::new(&unraw(i), i.span()));

    let key_tys = def
        .fields
        .iter()
        .map(|(_, ty)| quote!(<#ty as storm::UniqueValue>::Key))
        .collect::<Vec<_>>();

    let values = def
        .fields
        .iter()
        .map(|(i, ty)| quote!(<#ty as storm::UniqueValue>::unique_key(&entity.#i, Self::COLLATION, Self::TRIM)))
        .collect::<Vec<_>>();

    let (key_ty, value) = if def.fields.len() == 1 {
        (key_tys[0].clone(), values[0].clone())
    } else {
        (quote!((#(#key_tys,)*)), quote!((#(#values,)*)))
    };

    let collation = match def.collation {
        Collation::Exact => quote!(Exact),
        Collation::Ci => quote!(Ci),
        Collation::Ai => quote!(Ai),
        Collation::AiCi => quote!(AiCi),
    };

    let filter = def.filter.map(|f| {
        quote! {
            let filter: fn(&#entity) -> bool = #f;

            if !filter(entity) {
                return None;
            }
        }
    });

    let trim = def.trim;

    let entity_fields = def.fields.iter().map(|(i, _)| field_of(entity, i));

    let key_ty_of = quote!(<#marker as storm::Unique>::Key);
    let entity_key = quote!(<#entity as storm::Entity>::Key);

    if def.no_index && def.index.is_some() {
        return Err(Error::new(span, "`no_index` is incompatible with `index`.").to_compile_error());
    }

    // no_index: the marker finds the duplicates with a table scan.
    let (index_owner, index_items, index_init) = if def.no_index {
        (
            marker.clone(),
            quote! {
                impl storm::UniqueIndex<#key_ty_of, #entity_key> for #marker {
                    #[inline]
                    fn keys<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        value: &'a #key_ty_of,
                    ) -> storm::BoxFuture<'a, storm::Result<Vec<#entity_key>>> {
                        storm::unique::scan_keys::<Self>(trx, value)
                    }

                    #[inline]
                    fn first_other<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        value: &'a #key_ty_of,
                        key: &'a #entity_key,
                    ) -> storm::BoxFuture<'a, storm::Result<Option<#entity_key>>> {
                        storm::unique::scan_first_other::<Self>(trx, value, key)
                    }
                }
            },
            TokenStream::new(),
        )
    } else {
        let adapt_impl = hash_flat_set_adapt(
            vis,
            &adapt,
            entity,
            key_ty_of.clone(),
            quote!(<#marker as storm::Unique>::unique_key(entity)),
        );

        (
            adapt.clone(),
            quote! {
                #adapt_impl

                impl storm::UniqueAdapt for #adapt {
                    type Unique = #marker;
                }

                impl storm::UniqueIndex<#key_ty_of, #entity_key> for #adapt {
                    #[inline]
                    fn keys<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        value: &'a #key_ty_of,
                    ) -> storm::BoxFuture<'a, storm::Result<Vec<#entity_key>>> {
                        storm::unique::hash_index_keys::<Self>(trx, value)
                    }

                    #[inline]
                    fn first_other<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        value: &'a #key_ty_of,
                        key: &'a #entity_key,
                    ) -> storm::BoxFuture<'a, storm::Result<Option<#entity_key>>> {
                        storm::unique::hash_index_first_other::<Self>(trx, value, key)
                    }
                }

                /// Unique index: unique key -> entity keys.
                ///
                /// Search it with `lookup(&value)`, the collation and trimming are applied.
                #vis type #index = storm::indexing::HashFlatSetIndex<#adapt>;
            },
            quote!(<#adapt as storm::indexing::HashFlatSetAdapt>::register();),
        )
    };

    let items = quote! {
        /// Unique index marker generated by `#[storm(unique(...))]`.
        #vis struct #marker;

        impl storm::Unique for #marker {
            type Entity = #entity;
            type Key = #key_ty;
            type Index = #index_owner;

            const NAME: &'static str = #name_lit;
            const FIELDS: &'static [&'static str] = &[#(#field_lits),*];
            const COLLATION: storm::Collation = storm::Collation::#collation;
            const TRIM: bool = #trim;

            fn unique_key(entity: &Self::Entity) -> Option<Self::Key> {
                #filter
                Some(#value)
            }

            fn entity_fields() -> Vec<Box<dyn storm::Fields>> {
                [#(#entity_fields),*].into_iter().flatten().collect()
            }
        }

        #index_items
    };

    let init = quote! {
        #index_init
        <#entity as storm::EntityAccessor>::validating().on(storm::unique::check_validating::<#marker>);
    };

    Ok(Constraint { items, init })
}

fn unraw(ident: &Ident) -> String {
    let s = ident.to_string();
    s.strip_prefix("r#").map(str::to_owned).unwrap_or(s)
}

/// The index type name set by `index = "..."`, or the default one.
fn index_ident(
    index: Option<&SpannedValue<String>>,
    default: impl FnOnce() -> Ident,
) -> Result<Ident, TokenStream> {
    let Some(i) = index else {
        return Ok(default());
    };

    syn::parse_str::<Ident>(i)
        .map(|id| Ident::new(&id.to_string(), i.span()))
        .map_err(|_| {
            Error::new(
                i.span(),
                format!("`{}` is not a valid index type name.", **i),
            )
            .to_compile_error()
        })
}

/// Unique indexes and foreign keys declared on the model, as `storm_mssql::UniqueSchema` and
/// `storm_mssql::ForeignKeySchema` expressions, used to compare the model with the database.
///
/// Invalid attributes are skipped: the `Ctx` derive reports them.
#[cfg(feature = "mssql")]
pub(crate) fn mssql_schema(
    input: &DeriveInput,
    column_of: &dyn Fn(&Ident) -> Option<String>,
) -> (Vec<TokenStream>, Vec<TokenStream>) {
    let mut uniques = Vec::new();
    let mut foreign_keys = Vec::new();

    let Ok(fields) = input.fields() else {
        return (uniques, foreign_keys);
    };

    let mut add_unique = |def: UniqueDef| {
        let Some(columns) = def
            .fields
            .iter()
            .map(|(i, _)| column_of(i))
            .collect::<Option<Vec<_>>>()
        else {
            return;
        };

        let name = LitStr::new(&def.name.0, Span::call_site());
        let ci = matches!(def.collation, Collation::Ci | Collation::AiCi);
        let ai = matches!(def.collation, Collation::Ai | Collation::AiCi);
        let filtered = def.filter.is_some();

        uniques.push(quote! {
            storm_mssql::UniqueSchema {
                name: #name,
                columns: &[#(#columns),*],
                case_insensitive: #ci,
                accent_insensitive: #ai,
                filtered: #filtered,
            }
        });
    };

    for field in fields {
        let (Ok(args), Some(ident)) = (FieldArgs::from_field(field), &field.ident) else {
            continue;
        };

        if let (Some(fk), Some(column)) = (&args.foreign_key, column_of(ident)) {
            let table = match (&fk.entity, &fk.table) {
                (Some(parent), None) => quote!(<#parent as storm_mssql::MssqlMeta>::TABLE),
                (None, Some(table)) => {
                    let table = LitStr::new(table, Span::call_site());
                    quote!(#table)
                }
                _ => continue,
            };

            let field_name = unraw(ident);
            let on_delete = fk.on_delete.variant();

            foreign_keys.push(quote! {
                storm_mssql::ForeignKeySchema {
                    field: #field_name,
                    column: #column,
                    table: #table,
                    on_delete: storm::OnDelete::#on_delete,
                }
            });
        }

        if let Some(unique_args) = &args.unique {
            let default = UniqueFieldArgs::default();
            let unique_args = match unique_args {
                Override::Inherit => &default,
                Override::Explicit(a) => a,
            };

            add_unique(UniqueDef {
                name: (
                    unique_args
                        .name
                        .as_ref()
                        .map_or_else(|| unraw(ident), |n| n.as_str().to_owned()),
                    Span::call_site(),
                ),
                index: None,
                fields: vec![(ident, &field.ty)],
                collation: unique_args.collation,
                filter: unique_args.filter.as_ref(),
                trim: args.trim,
                no_index: unique_args.no_index,
            });
        }
    }

    if let Ok(args) = TypeArgs::from_derive_input(input) {
        for unique_args in &args.unique {
            if let Ok(def) = unique_def(fields, unique_args) {
                add_unique(def);
            }
        }

        for fk in &args.foreign_key {
            let table = match (&fk.entity, &fk.table) {
                (Some(parent), None) => quote!(<#parent as storm_mssql::MssqlMeta>::TABLE),
                (None, Some(table)) => {
                    let table = LitStr::new(table, Span::call_site());
                    quote!(#table)
                }
                _ => continue,
            };
            let column = LitStr::new(&fk.column, Span::call_site());
            let on_delete = fk.on_delete.variant();

            foreign_keys.push(quote! {
                storm_mssql::ForeignKeySchema {
                    field: #column,
                    column: #column,
                    table: #table,
                    on_delete: storm::OnDelete::#on_delete,
                }
            });
        }
    }

    (uniques, foreign_keys)
}

fn schema_only_foreign_key(field: &FieldArgs, args: &ForeignKeyArgs) -> Result<Constraint, TokenStream> {
    let span = field.ident.as_ref().map_or_else(|| field.ty.span(), |i| i.span());

    if args.entity.is_some() == args.table.is_some() {
        return Err(Error::new(span, "foreign_key requires either `entity` or `table`.").to_compile_error());
    }

    if args.hash || args.no_index || args.index.is_some() || args.flat_index.is_some() {
        return Err(Error::new(
            span,
            "`schema_only` is incompatible with `index`, `flat_index`, `hash` and `no_index`.",
        )
        .to_compile_error());
    }

    // no code uses the field: a wrapper of an `Option` (ex: `EqTrue<Option<T>>`) is accepted for `set_null`.
    Ok(Constraint {
        items: TokenStream::new(),
        init: TokenStream::new(),
    })
}

fn foreign_key(
    input: &DeriveInput,
    field: &FieldArgs,
    args: &ForeignKeyArgs,
) -> Result<Constraint, TokenStream> {
    let vis = &input.vis;
    let entity = &input.ident;

    let Some(field_ident) = &field.ident else {
        return Err(Error::new(
            field.ty.span(),
            "foreign_key is only supported on named fields.",
        )
        .to_compile_error());
    };

    // `table`: the parent is in a crate depending on this one, it registers the foreign key.
    let parent = match (&args.entity, &args.table) {
        (Some(parent), None) => Some(parent),
        (None, Some(_)) => None,
        _ => {
            return Err(Error::new(
                field_ident.span(),
                "foreign_key requires either `entity` or `table`.",
            )
            .to_compile_error());
        }
    };

    let is_option = is_option(&field.ty);

    if args.on_delete == OnDelete::SetNull && !is_option {
        return Err(Error::new(
            field.ty.span(),
            "on_delete = \"set_null\" requires an `Option` field.",
        )
        .to_compile_error());
    }

    let field_name = field_ident.to_string();
    let field_name = field_name.strip_prefix("r#").unwrap_or(&field_name);
    let field_lit = LitStr::new(field_name, field_ident.span());
    let prefix = format!("{entity}{}Fk", field_name.to_pascal_case());
    let fk = Ident::new(&prefix, field_ident.span());

    if let (Some(_), Some(flat)) = (&args.index, &args.flat_index) {
        return Err(
            Error::new(flat.span(), "use either `index` or `flat_index`.").to_compile_error(),
        );
    }

    if let (true, Some(flat)) = (args.hash, &args.flat_index) {
        return Err(
            Error::new(flat.span(), "use `index` with `hash`.").to_compile_error(),
        );
    }

    if args.no_index && (args.hash || args.index.is_some() || args.flat_index.is_some()) {
        return Err(Error::new(
            field_ident.span(),
            "`no_index` is incompatible with `index`, `flat_index` and `hash`.",
        )
        .to_compile_error());
    }

    let flat = !args.hash;
    let index = index_ident(args.flat_index.as_ref().or(args.index.as_ref()), || {
        Ident::new(&format!("{prefix}Index"), field_ident.span())
    })?;
    let adapt = Ident::new(&format!("{prefix}IndexAdapt"), field_ident.span());

    let get_fk = if is_option {
        quote!(child.#field_ident.as_ref())
    } else {
        quote!(Some(&child.#field_ident))
    };

    let (fk_trait, set_null_trait, parent_key, fk_ty, parent_ty) = match parent {
        Some(parent) => (
            quote!(storm::ForeignKey),
            quote!(storm::foreign_key::ForeignKeySetNull),
            quote!(<#parent as storm::Entity>::Key),
            quote!(#fk),
            quote!(#parent),
        ),
        None => {
            let ty = option_inner(&field.ty);

            (
                quote!(storm::foreign_key::ExternalForeignKey),
                quote!(storm::foreign_key::ExternalForeignKeySetNull),
                quote!(#ty),
                quote!(storm::foreign_key::External<Self, P>),
                quote!(P),
            )
        }
    };

    let (on_delete_variant, on_delete_init, plan_handler, set_null) = match args.on_delete {
        OnDelete::Cascade => (
            quote!(Cascade),
            quote!(<#parent_ty as storm::EntityAccessor>::removing().on(storm::foreign_key::on_delete_cascade::<#fk_ty>);),
            quote!(plan_cascade),
            quote!(),
        ),
        OnDelete::Restrict => (
            quote!(Restrict),
            quote!(storm::foreign_key::on_restrict::<#parent_ty>(storm::foreign_key::check_restrict::<#fk_ty>);),
            quote!(plan_restrict),
            quote!(),
        ),
        OnDelete::SetNull => (
            quote!(SetNull),
            quote!(<#parent_ty as storm::EntityAccessor>::removing().on(storm::foreign_key::on_delete_set_null::<#fk_ty>);),
            quote!(plan_set_null),
            quote! {
                impl #set_null_trait for #fk {
                    #[inline]
                    fn set_null(child: &mut Self::Child) {
                        child.#field_ident = None;
                    }

                    fn save<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        key: <Self::Child as storm::Entity>::Key,
                        child: Self::Child,
                    ) -> storm::BoxFuture<'a, storm::Result<()>> {
                        #[allow(unused_imports)]
                        use storm::foreign_key::{__FkSaveUpsert as _, __FkSaveUpsertMut as _};
                        (&storm::foreign_key::__FkSaver::<#entity>::new()).__fk_save(trx, key, child)
                    }
                }
            },
        ),
    };

    let child_key = quote!(<#entity as storm::Entity>::Key);

    // no reverse index: the marker finds the children with a table scan.
    let (index_owner, index_items, index_init) = if args.no_index {
        let scan = match parent {
            Some(_) => quote!(storm::foreign_key::scan_index_children::<Self>(trx, parent)),
            None => quote!(storm::foreign_key::scan_external_children::<Self>(trx, parent)),
        };

        (
            fk.clone(),
            quote! {
                impl storm::foreign_key::ForeignKeyIndex<#parent_key, #child_key> for #fk {
                    #[inline]
                    fn is_built(_ctx: &storm::Ctx) -> bool {
                        false
                    }

                    #[inline]
                    fn children<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        parent: &'a #parent_key,
                    ) -> storm::BoxFuture<'a, storm::Result<Vec<#child_key>>> {
                        #scan
                    }
                }
            },
            TokenStream::new(),
        )
    } else {
        let (adapt_impl, index_ty, adapt_trait, built, children) = if flat {
            (
                flat_set_adapt(
                    vis,
                    &adapt,
                    entity,
                    parent_key.clone(),
                    quote!(<#fk as #fk_trait>::foreign_key(id, entity).copied()),
                ),
                quote!(storm::indexing::FlatSetIndex<#adapt>),
                quote!(storm::indexing::FlatSetAdapt),
                quote!(flat_index_built),
                quote!(flat_index_children),
            )
        } else {
            (
                hash_flat_set_adapt(
                    vis,
                    &adapt,
                    entity,
                    parent_key.clone(),
                    quote!(<#fk as #fk_trait>::foreign_key(id, entity).cloned()),
                ),
                quote!(storm::indexing::HashFlatSetIndex<#adapt>),
                quote!(storm::indexing::HashFlatSetAdapt),
                quote!(hash_index_built),
                quote!(hash_index_children),
            )
        };

        (
            adapt.clone(),
            quote! {
                #adapt_impl

                impl storm::foreign_key::ForeignKeyIndex<#parent_key, #child_key> for #adapt {
                    #[inline]
                    fn is_built(ctx: &storm::Ctx) -> bool {
                        storm::foreign_key::#built::<Self>(ctx)
                    }

                    #[inline]
                    fn children<'a>(
                        trx: &'a mut storm::CtxTransaction<'_>,
                        parent: &'a #parent_key,
                    ) -> storm::BoxFuture<'a, storm::Result<Vec<#child_key>>> {
                        storm::foreign_key::#children::<Self>(trx, parent)
                    }
                }

                /// Reverse index of the foreign key: parent key -> child keys.
                #vis type #index = #index_ty;
            },
            quote!(<#adapt as #adapt_trait>::register();),
        )
    };

    let entity_field = field_of(entity, field_ident);
    let indexed = args.index.is_some() || args.flat_index.is_some();

    let handlers = quote! {
        <#entity as storm::EntityAccessor>::validating().on(storm::foreign_key::check_validating::<#fk_ty>);
        #on_delete_init
        storm::delete_plan::on_planning::<#parent_ty>(storm::delete_plan::#plan_handler::<#fk_ty>);
    };

    let (parent_item, register, init) = match parent {
        Some(parent) => (
            quote!(type Parent = #parent;),
            TokenStream::new(),
            quote! {
                #index_init
                #handlers
            },
        ),
        None => (
            quote!(type ParentKey = #parent_key;),
            quote! {
                impl #fk {
                    /// Enforces the foreign key referencing `P`.
                    ///
                    /// Called by `#[storm(referenced_by = ...)]` on the parent.
                    #vis fn register<P>()
                    where
                        P: storm::EntityAccessor + storm::CtxTypeInfo + storm::Entity<Key = #parent_key>,
                        storm::ProviderContainer: storm::provider::LoadAll<P, (), <P as storm::EntityAccessor>::Tbl>,
                    {
                        #handlers
                    }
                }
            },
            index_init,
        ),
    };

    let fk_doc = match parent {
        Some(_) => "Foreign key marker generated by `#[storm(foreign_key(...))]`.",
        None => {
            "Foreign key marker generated by `#[storm(foreign_key(table = ...))]`, registered by the parent with `#[storm(referenced_by = ...)]`."
        }
    };

    Ok(Constraint {
        items: quote! {
            #[doc = #fk_doc]
            #vis struct #fk;

            impl #fk_trait for #fk {
                type Child = #entity;
                #parent_item
                type Index = #index_owner;

                const FIELD: &'static str = #field_lit;
                const ON_DELETE: storm::OnDelete = storm::OnDelete::#on_delete_variant;
                const INDEXED: bool = #indexed;

                #[inline]
                fn foreign_key<'a>(
                    _key: &'a <Self::Child as storm::Entity>::Key,
                    child: &'a Self::Child,
                ) -> Option<&'a #parent_key> {
                    #get_fk
                }

                fn entity_field() -> Option<Box<dyn storm::Fields>> {
                    #entity_field
                }
            }

            #register

            #set_null

            #index_items
        },
        init,
    })
}

/// Foreign key on a column of the key (`#[storm(foreign_key(column = ...))]` on the type).
///
/// The child is found by key for a single key, with a table scan for a composite key
/// (tuple keys cannot be stored in a `FlatSetIndex`).
fn key_foreign_key(input: &DeriveInput, args: &KeyForeignKeyArgs) -> Result<Constraint, TokenStream> {
    let vis = &input.vis;
    let entity = &input.ident;
    let span = args.column.span();

    if args.schema_only {
        if args.entity.is_some() == args.table.is_some() {
            return Err(Error::new(span, "foreign_key requires either `entity` or `table`.").to_compile_error());
        }

        if args.key.is_some() {
            return Err(Error::new(span, "`schema_only` is incompatible with `key`.").to_compile_error());
        }

        return Ok(Constraint {
            items: TokenStream::new(),
            init: TokenStream::new(),
        });
    }

    let Some(parent) = args.entity.as_ref().filter(|_| args.table.is_none()) else {
        return Err(Error::new(
            span,
            "foreign_key on the type requires `entity`, `table` is only supported with `schema_only`.",
        )
        .to_compile_error());
    };

    let column = args.column.as_str();
    let column_lit = LitStr::new(column, span);
    let fk = Ident::new(&format!("{entity}{}Fk", column.to_pascal_case()), span);

    let (on_delete_variant, on_delete_init, plan_handler) = match args.on_delete {
        OnDelete::Cascade => (
            quote!(Cascade),
            quote!(<#parent as storm::EntityAccessor>::removing().on(storm::foreign_key::on_delete_cascade::<#fk>);),
            quote!(plan_cascade),
        ),
        OnDelete::Restrict => (
            quote!(Restrict),
            quote!(storm::foreign_key::on_restrict::<#parent>(storm::foreign_key::check_restrict::<#fk>);),
            quote!(plan_restrict),
        ),
        OnDelete::SetNull => {
            return Err(Error::new(
                span,
                "on_delete = \"set_null\" is not supported on a key column.",
            )
            .to_compile_error());
        }
    };

    let (get_fk, by_key, children) = match args.key {
        Some(i) => {
            let i = syn::Index::from(i);
            (quote!(Some(&key.#i)), false, quote!(scan_index_children))
        }
        None => (quote!(Some(key)), true, quote!(key_children)),
    };

    let parent_key = quote!(<#parent as storm::Entity>::Key);
    let child_key = quote!(<#entity as storm::Entity>::Key);

    Ok(Constraint {
        items: quote! {
            /// Foreign key marker generated by `#[storm(foreign_key(column = ...))]`.
            #vis struct #fk;

            impl storm::ForeignKey for #fk {
                type Child = #entity;
                type Parent = #parent;
                type Index = #fk;

                const FIELD: &'static str = #column_lit;
                const ON_DELETE: storm::OnDelete = storm::OnDelete::#on_delete_variant;
                const INDEXED: bool = #by_key;

                #[inline]
                fn foreign_key<'a>(
                    key: &'a #child_key,
                    _child: &'a Self::Child,
                ) -> Option<&'a #parent_key> {
                    #get_fk
                }
            }

            impl storm::foreign_key::ForeignKeyIndex<#parent_key, #child_key> for #fk {
                #[inline]
                fn is_built(_ctx: &storm::Ctx) -> bool {
                    #by_key
                }

                #[inline]
                fn children<'a>(
                    trx: &'a mut storm::CtxTransaction<'_>,
                    parent: &'a #parent_key,
                ) -> storm::BoxFuture<'a, storm::Result<Vec<#child_key>>> {
                    storm::foreign_key::#children::<Self>(trx, parent)
                }
            }
        },
        init: quote! {
            <#entity as storm::EntityAccessor>::validating().on(storm::foreign_key::check_validating::<#fk>);
            #on_delete_init
            storm::delete_plan::on_planning::<#parent>(storm::delete_plan::#plan_handler::<#fk>);
        },
    })
}

/// Generates a `HashFlatSetAdapt` indexing each entity under the `Option<K>` returned by
/// `key` (an expression using `entity`).
fn hash_flat_set_adapt(
    vis: &Visibility,
    adapt: &Ident,
    entity: &Ident,
    k: TokenStream,
    key: TokenStream,
) -> TokenStream {
    let index_var = ctx_var(quote!(storm::indexing::HashFlatSetIndex<#adapt>));
    let events = adapt_events(adapt);

    quote! {
        #[doc(hidden)]
        #vis struct #adapt;

        impl storm::indexing::HashFlatSetAdapt for #adapt {
            type Entity = #entity;
            type K = #k;
            type V = <#entity as storm::Entity>::Key;

            fn adapt(
                id: &<Self::Entity as storm::Entity>::Key,
                entity: &Self::Entity,
                out: &mut storm::rustc_hash::FxHashSet<(Option<Self::K>, Self::V)>,
            ) {
                if let Some(k) = #key {
                    out.insert((Some(k), Clone::clone(id)));
                }
            }

            #[inline]
            fn index_var() -> storm::CtxVar<storm::indexing::HashFlatSetIndex<Self>> {
                #index_var
            }
        }

        #events
    }
}

/// Generates a `FlatSetAdapt` indexing each entity under the `Option<K>` returned by `key`
/// (an expression using `entity`), `None` included.
fn flat_set_adapt(
    vis: &Visibility,
    adapt: &Ident,
    entity: &Ident,
    k: TokenStream,
    key: TokenStream,
) -> TokenStream {
    let index_var = ctx_var(quote!(storm::indexing::FlatSetIndex<#adapt>));
    let events = adapt_events(adapt);

    quote! {
        #[doc(hidden)]
        #vis struct #adapt;

        impl storm::indexing::FlatSetAdapt for #adapt {
            type Entity = #entity;
            type K = #k;
            type V = <#entity as storm::Entity>::Key;

            fn adapt(
                id: &<Self::Entity as storm::Entity>::Key,
                entity: &Self::Entity,
                out: &mut storm::rustc_hash::FxHashSet<(Option<Self::K>, Self::V)>,
            ) {
                out.insert((#key, *id));
            }

            #[inline]
            fn index_var() -> storm::CtxVar<storm::indexing::FlatSetIndex<Self>> {
                #index_var
            }
        }

        #events
    }
}

fn adapt_events(adapt: &Ident) -> TokenStream {
    quote! {
        impl storm::Clearable for #adapt {
            #[inline]
            fn cleared() -> &'static storm::ClearEvent {
                static E: storm::ClearEvent = storm::ClearEvent::new();
                &E
            }
        }

        impl storm::Touchable for #adapt {
            #[inline]
            fn touched() -> &'static storm::TouchedEvent {
                static E: storm::TouchedEvent = storm::TouchedEvent::new();
                &E
            }
        }
    }
}

fn is_option(ty: &Type) -> bool {
    match ty {
        Type::Path(p) if p.qself.is_none() => p.path.segments.last().is_some_and(|s| {
            s.ident == "Option" && matches!(s.arguments, PathArguments::AngleBracketed(_))
        }),
        Type::Group(g) => is_option(&g.elem),
        Type::Paren(p) => is_option(&p.elem),
        _ => false,
    }
}

/// `T` of an `Option<T>`, else the type itself.
fn option_inner(ty: &Type) -> &Type {
    match ty {
        Type::Path(p) if p.qself.is_none() => p
            .path
            .segments
            .last()
            .filter(|s| s.ident == "Option")
            .and_then(|s| match &s.arguments {
                PathArguments::AngleBracketed(a) => a.args.first(),
                _ => None,
            })
            .and_then(|a| match a {
                GenericArgument::Type(t) => Some(t),
                _ => None,
            })
            .unwrap_or(ty),
        Type::Group(g) => option_inner(&g.elem),
        Type::Paren(p) => option_inner(&p.elem),
        _ => ty,
    }
}

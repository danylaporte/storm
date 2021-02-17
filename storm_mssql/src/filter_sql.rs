use std::borrow::Cow;
use tiberius::ToSql;

/// Allow to filter a list of rows.
///
/// # Implement FilterSql
/// ```
/// use std::borrow::Cow;
/// use storm_mssql::FilterSql;
/// use tiberius::ToSql;
///
/// type TopicId = i32;
///
/// struct CommentPerTopicId(TopicId);
///
/// impl FilterSql for CommentPerTopicId {
///     fn filter_sql(&self, param_index: usize) -> (Cow<'_, str>, Cow<'_, [&'_ dyn ToSql]>) {
///         (
///             Cow::Owned(format!("[TopicId] = @p{}", param_index)),
///             Cow::Owned(vec![&self.0 as _]),
///         )
///     }
/// }
/// ```
pub trait FilterSql: Send + Sync {
    fn filter_sql(&self, param_index: usize) -> (Cow<'_, str>, Cow<'_, [&'_ dyn ToSql]>);
}

impl FilterSql for () {
    fn filter_sql(&self, _: usize) -> (Cow<'_, str>, Cow<'_, [&'_ dyn ToSql]>) {
        (Cow::Borrowed(""), Cow::Borrowed(&[]))
    }
}

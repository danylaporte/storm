use crate::ToSql;
use std::borrow::Cow;

/// Allow to filter a list of rows.
///
/// # Implement FilterSql
/// ```
/// use std::borrow::Cow;
/// use storm_mssql::{FilterSql, ToSql};
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

impl FilterSql for (&str, &[&'_ dyn ToSql]) {
    fn filter_sql(&self, _: usize) -> (Cow<'_, str>, Cow<'_, [&'_ dyn ToSql]>) {
        (Cow::Borrowed(self.0), Cow::Borrowed(self.1))
    }
}

pub struct KeysFilter<'a, K>(pub &'a str, pub &'a [K]);

impl<K> FilterSql for KeysFilter<'_, K>
where
    K: ToSql,
{
    fn filter_sql(&self, param_index: usize) -> (Cow<'_, str>, Cow<'_, [&'_ dyn ToSql]>) {
        use std::fmt::Write;

        let mut s = String::with_capacity(self.0.len() + self.1.len() * 5 + 5);
        s.push_str(self.0);
        s.push_str(" IN (");

        for index in 0..self.1.len() {
            if index != 0 {
                s.push(',');
            }

            // fmt::Write for String is infallible.
            let _ = write!(&mut s, "@p{}", index + 1 + param_index);
        }

        s.push(')');

        (
            Cow::Owned(s),
            Cow::Owned(self.1.iter().map(|v| v as &dyn ToSql).collect()),
        )
    }
}

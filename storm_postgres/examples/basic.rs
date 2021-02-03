use storm_postgres::{FromSql, ToSql, Upsert};

fn main() {}

#[derive(Debug, FromSql, ToSql)]
pub struct UserId(u32);

impl From<UserId> for usize {
    fn from(u: UserId) -> Self {
        u.0 as _
    }
}

#[derive(Upsert)]
#[table("public.users")]
pub struct UserRow {
    #[key]
    id: UserId,
    name: String,
}

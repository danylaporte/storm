use crate::{EntityObj, GetOwned, ObjBase, Result, Trx};

pub enum ChangeState<T> {
    New(T),
    Changed { old: T, new: T },
}

impl<T> ChangeState<T> {
    pub async fn from_trx<'a, E, F, Q>(
        trx: &'a mut Trx<'a>,
        q: &Q,
        new: &E,
        map: F,
    ) -> Result<Option<Self>>
    where
        F: Fn(&E) -> T,
        E: EntityObj,
        T: PartialEq,
        <E::Tbl as ObjBase>::Trx<'a>: GetOwned<'a, E, Q>,
    {
        let old = trx.get_entity(q).await?;

        Ok(Self::from_new_old(new, old, map))
    }

    pub fn from_new_old<E, F>(new: &E, old: Option<&E>, map: F) -> Option<Self>
    where
        F: Fn(&E) -> T,
        E: EntityObj,
        T: PartialEq,
    {
        match old {
            Some(old) => {
                let old = map(old);
                let new = map(new);

                if old == new {
                    None
                } else {
                    Some(ChangeState::Changed { old, new })
                }
            }
            None => Some(ChangeState::New(map(new))),
        }
    }
}

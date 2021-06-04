use crate::{BoxFuture, Entity, Result};

pub trait Insert<E: Entity> {
    fn insert(&mut self, k: E::Key, v: E) -> BoxFuture<'_, Result<()>>;

    fn insert_all<'a, I>(&'a mut self, iter: I) -> BoxFuture<'a, Result<()>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Send,
    {
        Box::pin(async move {
            for (k, v) in iter {
                self.insert(k, v).await?;
            }

            Ok(())
        })
    }
}

pub trait InsertMut<E: Entity> {
    fn insert_mut(&mut self, k: E::Key, v: E) -> BoxFuture<'_, Result<E::Key>>;

    fn insert_mut_all<'a, I>(&'a mut self, iter: I) -> BoxFuture<'a, Result<()>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Send,
    {
        Box::pin(async move {
            for (k, v) in iter {
                self.insert_mut(k, v).await?;
            }

            Ok(())
        })
    }
}

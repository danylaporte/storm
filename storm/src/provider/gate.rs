use async_trait::async_trait;

#[async_trait]
pub trait Gate<'a> {
    type Gate: Send;

    async fn gate(&'a self) -> Self::Gate;
}

#[async_trait]
impl<'a, T> Gate<'a> for &T
where
    T: Gate<'a> + Send + Sync,
{
    type Gate = T::Gate;

    async fn gate(&'a self) -> Self::Gate {
        (*self).gate().await
    }
}

#[cfg(feature = "tokio")]
#[async_trait]
impl<'a> Gate<'a> for () {
    type Gate = tokio::sync::MutexGuard<'a, ()>;

    async fn gate(&'a self) -> Self::Gate {
        static GATE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        GATE.lock().await
    }
}

#[cfg(feature = "tokio")]
#[async_trait]
impl<'a> Gate<'a> for tokio::sync::Mutex<()> {
    type Gate = tokio::sync::MutexGuard<'a, ()>;

    async fn gate(&'a self) -> Self::Gate {
        self.lock().await
    }
}

pub trait GetVersion {
    #[must_use]
    fn get_version(&self) -> Option<u64>;
}

impl<T> GetVersion for &T
where
    T: GetVersion,
{
    fn get_version(&self) -> Option<u64> {
        (**self).get_version()
    }
}

impl<T> GetVersion for Option<T>
where
    T: GetVersion,
{
    fn get_version(&self) -> Option<u64> {
        self.as_ref()?.get_version()
    }
}

use std::cmp::max;

pub trait GetVersion {
    #[must_use]
    fn get_version(&self) -> u64;

    fn max(self, version: &mut u64) -> Self
    where
        Self: Sized,
    {
        *version = max(self.get_version(), *version);
        self
    }
}

impl<T> GetVersion for &T
where
    T: GetVersion,
{
    fn get_version(&self) -> u64 {
        (**self).get_version()
    }
}

pub trait GetVersionOpt {
    #[must_use]
    fn get_version_opt(&self) -> Option<u64>;
}

impl<T> GetVersionOpt for &T
where
    T: GetVersionOpt,
{
    fn get_version_opt(&self) -> Option<u64> {
        (**self).get_version_opt()
    }
}

impl<T> GetVersionOpt for Option<T>
where
    T: GetVersionOpt,
{
    fn get_version_opt(&self) -> Option<u64> {
        self.as_ref()?.get_version_opt()
    }
}

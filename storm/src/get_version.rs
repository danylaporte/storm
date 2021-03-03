pub trait GetVersion {
    #[must_use]
    fn get_version(&self) -> Option<u64>;
}

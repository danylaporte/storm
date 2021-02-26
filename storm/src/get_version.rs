pub trait GetVersion {
    #[must_use]
    fn get_version(&self) -> u64;
}

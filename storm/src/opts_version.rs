pub trait OptsVersion {
    fn opts_new_version(&mut self) -> u64;

    fn opts_version(&self) -> u64;
}

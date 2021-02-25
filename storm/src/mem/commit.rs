pub trait Commit {
    type Log;

    #[must_use]
    fn commit(self) -> Self::Log;
}

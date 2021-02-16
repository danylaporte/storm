pub trait Commit {
    type Log;
    fn commit(self) -> Self::Log;
}

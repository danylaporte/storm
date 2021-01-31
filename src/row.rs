pub trait Row {
    type Key;
    fn key(&self) -> Self::Key;
}

pub trait Transaction<'a> {
    type Transaction;

    fn transaction(&'a self) -> Self::Transaction;
}

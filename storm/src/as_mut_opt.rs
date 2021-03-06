pub trait AsMutOpt<T> {
    fn as_mut_opt(&mut self) -> Option<&mut T>;
}

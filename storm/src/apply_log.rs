pub trait ApplyLog<L> {
    fn apply_log(&mut self, log: L) -> bool;
}

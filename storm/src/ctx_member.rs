use once_cell::sync::OnceCell;

pub trait CtxMember {
    type Member;
}

impl<T> CtxMember for OnceCell<T> {
    type Member = T;
}

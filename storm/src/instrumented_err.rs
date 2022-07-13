use std::{
    fmt::Debug,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tracing::Span;

pin_project_lite::pin_project! {
    #[derive(Debug, Clone)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct InstrumentedErr<F> {
        #[pin]
        fut: F,
        span: Span,
    }
}

impl<F, T, E> Future for InstrumentedErr<F>
where
    F: Future<Output = Result<T, E>>,
    E: Debug,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let p = self.project();
        let _ = p.span.enter();

        p.fut.poll(cx).map_err(|e| {
            p.span.record("err", &tracing::field::debug(&e));
            e
        })
    }
}

pub trait InstrumentErr: Sized {
    fn instrument_err(self, span: Span) -> InstrumentedErr<Self> {
        InstrumentedErr { fut: self, span }
    }
}

impl<T: Sized> InstrumentErr for T {}

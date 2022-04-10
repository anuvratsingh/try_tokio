use core::fmt;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use pin_project::pin_project;
use tokio::time::Sleep;
use tower::Service;

#[derive(Debug, Clone)]
struct Timeout<T> {
    inner: T,
    timeout: Duration,
}

#[pin_project]
struct ResponseFuture<F> {
    #[pin]
    res_future: F,
    #[pin]
    sleep: Sleep,
}

// enum TimeoutError<Error> {
//     Timeout(InnerTimeoutError),
//     Service(Error),
// }

#[derive(Debug, Default)]
struct TimeoutError(());

type BoxError = Box<dyn std::error::Error + Send + Sync>;

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("request time out")
    }
}

impl std::error::Error for TimeoutError {}

impl<T> Timeout<T> {
    #[allow(dead_code)]
    fn new(inner: T, timeout: Duration) -> Self {
        Timeout { inner, timeout }
    }
}

impl<T, Request> Service<Request> for Timeout<T>
where
    T: Service<Request>,
    T::Error: Into<BoxError>
{
    type Response = T::Response;
    type Error = BoxError;
    type Future = ResponseFuture<T::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let res_future = self.inner.call(req);
        let sleep = tokio::time::sleep(self.timeout);

        ResponseFuture { res_future, sleep }
    }
}

impl<F, Response, Error> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, Error>>,
    Error: Into<BoxError>,
{
    type Output = Result<Response, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this.res_future.poll(cx) {
            Poll::Ready(result) => {
                let result = result.map_err(Into::into);
                return Poll::Ready(result);
            }
            Poll::Pending => {}
        }

        match this.sleep.poll(cx) {
            Poll::Ready(_) => {
                let err = Box::new(TimeoutError(()));
                return Poll::Ready(Err(err));
            }
            Poll::Pending => {}
        }

        Poll::Pending
    }
}

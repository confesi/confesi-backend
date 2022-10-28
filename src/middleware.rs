use std::future::{
	Ready,
	ready,
};
use actix_web::Error;
use actix_web::error::ErrorBadRequest;
use actix_web::dev::{
	Service,
	ServiceRequest,
	ServiceResponse,
	Transform,
	forward_ready,
};
use actix_web::http::header;
use futures::future::Either;

/// Permits requests only with the specified `Host` header.
#[derive(Clone, Copy, Debug)]
pub struct HostCheckWrap(pub &'static str);

impl<S, B> Transform<S, ServiceRequest> for HostCheckWrap
where
	S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
	S::Future: 'static,
	B: 'static,
{
	type Response = ServiceResponse<B>;
	type Error = Error;
	type InitError = ();
	type Transform = HostCheckMiddleware<S>;
	type Future = Ready<Result<Self::Transform, Self::InitError>>;

	fn new_transform(&self, service: S) -> Self::Future {
		ready(
			Ok(
				HostCheckMiddleware {
					permitted_host: self.0,
					service,
				}
			)
		)
	}
}

pub struct HostCheckMiddleware<S> {
	permitted_host: &'static str,
	service: S,
}

impl<S, B> Service<ServiceRequest> for HostCheckMiddleware<S>
where
	S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
	S::Future: 'static,
	B: 'static,
{
	type Response = ServiceResponse<B>;
	type Error = Error;
	type Future = Either<
		Ready<Result<Self::Response, Self::Error>>,
		S::Future,
	>;

	forward_ready!(service);

	fn call(&self, req: ServiceRequest) -> Self::Future {
		if req.headers().get(header::HOST).map(|v| v.as_bytes()) == Some(self.permitted_host.as_bytes()) {
			Either::Right(self.service.call(req))
		} else {
			Either::Left(ready(Err(ErrorBadRequest("Invalid Host header"))))
		}
	}
}

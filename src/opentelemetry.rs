//! opentelemetry integration
pub use opentelemetry::*;
pub use opentelemetry_sdk as sdk;
pub use tracing_opentelemetry;

use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry::propagation::text_map_propagator::TextMapPropagator;

///Opentelemetry extractor for [http::HeaderMap](https://docs.rs/http/latest/http/header/struct.HeaderMap.html)
pub struct HeaderMapExtractor<'a, T: AsRef<[u8]>>(pub &'a http::HeaderMap<T>);

impl<T: AsRef<[u8]>> opentelemetry::propagation::Extractor for HeaderMapExtractor<'_, T> {
    #[inline]
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| core::str::from_utf8(value.as_ref()).ok())
    }

    #[inline]
    fn keys(&self) -> Vec<&str> {
        Vec::new()
    }

    #[inline]
    fn get_all(&self, key: &str) -> Option<Vec<&str>> {
        let values: Vec<_> = self.0.get_all(key).iter().filter_map(|value| core::str::from_utf8(value.as_ref()).ok()).collect();
        if values.is_empty() {
            None
        } else {
            Some(values)
        }
    }
}

///Opentelemetry injector for [http::HeaderMap](https://docs.rs/http/latest/http/header/struct.HeaderMap.html)
pub struct HeaderMapInjector<'a, T: TryFrom<String>>(pub &'a mut http::HeaderMap<T>);

impl<T: TryFrom<String>> opentelemetry::propagation::Injector for HeaderMapInjector<'_, T> {
    #[inline]
    fn set(&mut self, key: &str, value: String) {
        let key = match http::HeaderName::from_bytes(key.as_bytes()) {
            Ok(key) => key,
            Err(_) => unreachable!()
        };
        match value.try_into() {
            Ok(value) => self.0.insert(key, value),
            Err(_) => unreachable!(),
        };
    }
}

#[inline(always)]
///Extracts OTEL context from `request` propagating it as `span`'s parent
///
///Note that this can only be done once for single span
pub fn on_request<T>(span: &tracing::Span, request: &http::Request<T>) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let propagator = TraceContextPropagator::new();
    let context = propagator.extract(&HeaderMapExtractor(request.headers()));

    if let Err(error) = span.set_parent(context) {
        tracing::warn!("Unable to propagate parent context: {error}");
    }
}

#[inline(always)]
///Propagates success into `span` context and then export context headers into response
pub fn on_response_ok<T>(span: &tracing::Span, response: &mut http::Response<T>) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    span.set_status(trace::Status::Ok);

    let propagator = TraceContextPropagator::new();
    let context = span.context();
    propagator.inject_context(&context, &mut HeaderMapInjector(response.headers_mut()));
}

#[inline(always)]
///Propagates error into `span` context
pub fn on_response_error(span: &tracing::Span, error: &impl std::error::Error) {
    use tracing_opentelemetry::OpenTelemetrySpanExt;

    let error = trace::Status::Error {
        description: error.to_string().into()
    };
    span.set_status(error);
}

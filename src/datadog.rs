//! datadog integration
//!
//! This is alternative to using specialized OTEL exporter: <https://crates.io/crates/opentelemetry-datadog>

pub use tracing_datadog;

#[inline(always)]
///Extracts datadog context from `request` propagating it as `span`'s parent
///
///Datadog format is somewhat different from opentelemetry:
///<https://docs.datadoghq.com/tracing/trace_collection/trace_context_propagation/?tab=java#datadog-format>
pub fn on_request<T>(span: &tracing::Span, request: &http::Request<T>) {
    let context = tracing_datadog::http::DatadogContext::from_w3c_headers(request.headers());
    tracing_datadog::http::DistributedTracingContext::set_context(span, context);
}

#[inline(always)]
///Propagates context into response's headers
pub fn on_response_ok<T>(span: &tracing::Span, response: &mut http::Response<T>) {
    let context = tracing_datadog::http::DistributedTracingContext::get_context(span);
    //rather inefficient way to do it but oh well...
    response.headers_mut().extend(context.to_w3c_headers())
}

#[inline(always)]
///No `error` propagation is done aside from default one
pub fn on_response_error(_span: &tracing::Span, _error: &impl std::error::Error) {
}

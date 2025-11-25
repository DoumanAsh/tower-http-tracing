//! datadog integration
//!
//! This is alternative to using specialized OTEL exporter: <https://crates.io/crates/opentelemetry-datadog>

use core::fmt;

pub use tracing_datadog;
use tracing_datadog::context::{self, DatadogContext, Strategy};

///W3C header name
pub const W3C_TRACEPARENT_NAME: http::HeaderName = http::HeaderName::from_static("traceparent");
const W3C_VERSION: u8 = 0;
const TRACE_SAMPLED_FLAG: u8 = 0x01;

///Propagation strategy for W3C header
pub struct Propagation;

struct BytesWriter(bytes::BytesMut);

impl BytesWriter {
    #[inline(always)]
    fn finish(self) -> bytes::Bytes {
        self.0.freeze()
    }
}

impl fmt::Write for BytesWriter {
    #[inline(always)]
    fn write_str(&mut self, input: &str) -> fmt::Result {
        self.0.extend_from_slice(input.as_bytes());
        Ok(())
    }
}

//A more efficient re-implementation of tracing_datadog's default impl
impl Strategy<http::HeaderMap> for Propagation {
    fn inject(headers: &mut http::HeaderMap, context: DatadogContext) {
        use fmt::Write;

        let DatadogContext { trace_id, parent_id } = &context;
        //Make DatadogContext::is_empty() public, also parent_id == 0 is fine?
        if *trace_id == 0 {
            return;
        }
        let mut out = BytesWriter(bytes::BytesMut::new());

        //Cannot fail, will panic on OOM
        let _ = write!(out, "{W3C_VERSION:02x}-{trace_id:032x}-{parent_id:016x}-{TRACE_SAMPLED_FLAG:02x}");

        let value = unsafe {
            //BytesWriter is guaranteed to only write via `fmt::Write` so all content is valid utf-8
            http::HeaderValue::from_maybe_shared_unchecked(out.finish())
        };
        headers.insert(W3C_TRACEPARENT_NAME, value);
    }

    fn extract(headers: &http::HeaderMap) -> DatadogContext {
        let header = match headers.get(W3C_TRACEPARENT_NAME).and_then(|value| value.to_str().ok()) {
            Some(header) => header,
            None => return DatadogContext::default(),
        };

        let mut parts = header.split('-');

        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(version), Some(trace_id), Some(parent_id), Some(trace_flag)) => {
                if u8::from_str_radix(version, 16).map(|version| version != W3C_VERSION).unwrap_or(true) {
                    //Cannot recognize version, then we cannot parse it
                    return DatadogContext::default();
                }

                if u8::from_str_radix(trace_flag, 16).map(|flag| flag & TRACE_SAMPLED_FLAG != TRACE_SAMPLED_FLAG).unwrap_or(true) {
                    //Not sampled = no propagation
                    return DatadogContext::default();
                }

                match (u128::from_str_radix(trace_id, 16), u64::from_str_radix(parent_id, 16)) {
                    (Ok(trace_id), Ok(parent_id)) => DatadogContext {
                        trace_id,
                        parent_id,
                    },
                    _ => DatadogContext::default()
                }
            },
            _ => DatadogContext::default(),
        }
    }
}

#[inline(always)]
///Extracts datadog context from `request` propagating it as `span`'s parent
///
///Datadog format is somewhat different from opentelemetry:
///<https://docs.datadoghq.com/tracing/trace_collection/trace_context_propagation/?tab=java#datadog-format>
pub fn on_request<T>(span: &tracing::Span, request: &http::Request<T>) {
    let context = Propagation::extract(request.headers());
    context::TracingContextExt::set_context(span, context);
}

#[inline(always)]
///Propagates context into response's headers
pub fn on_response_ok<T>(span: &tracing::Span, response: &mut http::Response<T>) {
    let context = tracing_datadog::context::TracingContextExt::get_context(span);
    Propagation::inject(response.headers_mut(), context);
}

#[inline(always)]
///No `error` propagation is done aside from default one
pub fn on_response_error(_span: &tracing::Span, _error: &impl std::error::Error) {
}

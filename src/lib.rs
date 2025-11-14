//!Tower tracing middleware to annotate every HTTP request with tracing's span.
//!
//!## Span creation
//!
//!Use [macro](macro.make_request_spanner.html) to declare function that creates desirable span
//!
//!## Example
//!
//!Below is illustration of how to initialize request layer for passing into your service
//!
//!```rust
//!use std::net::IpAddr;
//!
//!use tower_http_tracing::HttpRequestLayer;
//!
//!//Logic to extract client ip has to be written by user
//!//You can use utilities in separate crate to design this logic:
//!//https://docs.rs/http-ip/latest/http_ip/
//!fn extract_client_ip(_parts: &http::request::Parts) -> Option<IpAddr> {
//!    None
//!}
//!tower_http_tracing::make_request_spanner!(make_my_request_span("my_request", tracing::Level::INFO));
//!let layer = HttpRequestLayer::new(make_my_request_span).with_extract_client_ip(extract_client_ip)
//!                                                       .with_inspect_headers(&[&http::header::FORWARDED]);
//!//Use above layer in your service
//!```

#![warn(missing_docs)]
#![allow(clippy::style)]

mod grpc;
mod headers;

use std::net::IpAddr;
use core::{cmp, fmt, ptr, task};
use core::pin::Pin;
use core::future::Future;

pub use tracing;

///RequestId's header name
pub const REQUEST_ID: http::HeaderName = http::HeaderName::from_static("x-request-id");
///Alias to function signature required to create span
pub type MakeSpan = fn() -> tracing::Span;
///ALias to function signature to extract client's ip from request
pub type ExtractClientIp = fn(&http::request::Parts) -> Option<IpAddr>;

#[inline]
fn default_client_ip(_: &http::request::Parts) -> Option<IpAddr> {
    None
}

#[derive(Copy, Clone, PartialEq, Eq)]
///Possible request protocol
pub enum Protocol {
    ///Regular HTTP call
    ///
    ///Default value for all requests
    Http,
    ///gRPC call, identified by presence of `Content-Type` with grpc protocol signature
    Grpc,
}

impl Protocol {
    #[inline(always)]
    ///Determines protocol from value of `Content-Type`
    pub fn from_content_type(typ: &[u8]) -> Self {
        if typ.starts_with(b"application/grpc") {
            Self::Grpc
        } else {
            Self::Http
        }
    }

    #[inline(always)]
    ///Returns textual representation of the `self`
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Grpc => "grpc",
            Self::Http => "http"
        }
    }
}

impl fmt::Debug for Protocol {
    #[inline(always)]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), fmt)
    }
}

impl fmt::Display for Protocol {
    #[inline(always)]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), fmt)
    }
}

type RequestIdBuffer = [u8; 64];

#[derive(Clone)]
///Request's id
///
///By default it is extracted from `X-Request-Id` header
pub struct RequestId {
    buffer: RequestIdBuffer,
    len: u8,
}

impl RequestId {
    fn from_bytes(bytes: &[u8]) -> Self {
        let mut buffer: RequestIdBuffer = [0; 64];

        let len = cmp::min(buffer.len(), bytes.len());

        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer.as_mut_ptr(), len)
        };

        Self {
            buffer,
            len: len as _,
        }
    }

    fn from_uuid(uuid: uuid::Uuid) -> Self {
        let mut buffer: RequestIdBuffer = [0; 64];
        let uuid = uuid.as_hyphenated();
        let len = uuid.encode_lower(&mut buffer).len();

        Self {
            buffer,
            len: len as _,
        }
    }

    #[inline]
    ///Returns slice to already written data.
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe {
            core::slice::from_raw_parts(self.buffer.as_ptr(), self.len as _)
        }
    }

    #[inline(always)]
    ///Gets textual representation of the request id, if header value is string
    pub const fn as_str(&self) -> Option<&str> {
        match core::str::from_utf8(self.as_bytes()) {
            Ok(header) => Some(header),
            Err(_) => None,
        }
    }
}

impl fmt::Debug for RequestId {
    #[inline(always)]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(id) => fmt::Debug::fmt(id, fmt),
            None => fmt::Debug::fmt(self.as_bytes(), fmt),
        }
    }
}

impl fmt::Display for RequestId {
    #[inline(always)]
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(id) => fmt::Display::fmt(id, fmt),
            None => fmt::Display::fmt("<non-utf8>", fmt),
        }
    }
}

#[macro_export]
///Declares `fn` function compatible with `MakeSpan` using provided parameters
///
///## Span fields
///
///Following fields are declared when span is created:
///- `http.request.method`
///- `url.path`
///- `url.query`
///- `url.scheme`
///- `http.request_id` - Inherited from request 'X-Request-Id' or random uuid
///- `user_agent.original` - Only populated if user agent header is present
///- `http.headers` - Optional. Populated if more than 1 header specified via layer [config](struct.HttpRequestLayer.html#method.with_inspect_headers)
///- `network.protocol.name` - Either `http` or `grpc` depending on `content-type`
///- `network.protocol.version` - Set to HTTP version in case of plain `http` protocol.
///- `client.address` - Optionally added if IP extractor is specified via layer [config](struct.HttpRequestLayer.html#method.with_extract_client_ip)
///- `http.response.status_code` - Semantics of this code depends on `protocol`
///- `error.type` - Populated with `core::any::type_name` value of error type used by the service.
///- `error.message` - Populated with `Display` content of the error, returned by underlying service, after processing request.
///
///Loosely follows <https://opentelemetry.io/docs/specs/semconv/http/http-spans/#http-server>
///
///## Usage
///
///```
///use tower_http_tracing::make_request_spanner;
///
///make_request_spanner!(make_my_request_span("my_request", tracing::Level::INFO));
/////Customize span with extra fields. You can use tracing::field::Empty if you want to omit value
///make_request_spanner!(make_my_service_request_span("my_request", tracing::Level::INFO, service_name = "<your name>"));
///
///let span = make_my_request_span();
///span.record("url.path", "I can override span field");
///
///```
macro_rules! make_request_spanner {
    ($fn:ident($name:literal, $level:expr)) => {
        $crate::make_request_spanner!($fn($name, $level,));
    };
    ($fn:ident($name:literal, $level:expr, $($fields:tt)*)) => {
        #[track_caller]
        pub fn $fn() -> $crate::tracing::Span {
            use $crate::tracing::field;

            $crate::tracing::span!(
                $level,
                $name,
                //Assigned on creation of span
                http.request.method = field::Empty,
                url.path = field::Empty,
                url.query = field::Empty,
                url.scheme = field::Empty,
                http.request_id = field::Empty,
                user_agent.original = field::Empty,
                http.headers = field::Empty,
                network.protocol.name = field::Empty,
                network.protocol.version = field::Empty,
                //Optional
                client.address = field::Empty,
                //Assigned after request is complete
                http.response.status_code = field::Empty,
                error.message = field::Empty,
                $(
                    $fields
                )*
            )
        }
    };
}

#[derive(Clone, Debug)]
///Request's information
///
///It is accessible via [extensions](https://docs.rs/http/latest/http/struct.Extensions.html)
pub struct RequestInfo {
    ///Request's protocol
    pub protocol: Protocol,
    ///Request's id
    pub request_id: RequestId,
    ///Client's IP address extracted, if available.
    pub client_ip: Option<IpAddr>,
}

///Request's span information
///
///Created on every request by the middleware, but not accessible to the user directly
pub struct RequestSpan {
    ///Underlying tracing span
    pub span: tracing::Span,
    ///Request's information
    pub info: RequestInfo,
}

impl RequestSpan {
    ///Creates new request span
    pub fn new(span: tracing::Span, extract_client_ip: ExtractClientIp, parts: &http::request::Parts) -> Self {
        let _entered = span.enter();

        let client_ip = (extract_client_ip)(parts);
        let protocol = parts.headers
                            .get(http::header::CONTENT_TYPE)
                            .map_or(Protocol::Http, |content_type| Protocol::from_content_type(content_type.as_bytes()));

        let request_id = if let Some(request_id) = parts.headers.get(REQUEST_ID) {
            RequestId::from_bytes(request_id.as_bytes())
        } else {
            RequestId::from_uuid(uuid::Uuid::new_v4())
        };

        if let Some(user_agent) = parts.headers.get(http::header::USER_AGENT).and_then(|header| header.to_str().ok()) {
            span.record("user_agent.original", user_agent);
        }
        span.record("http.request.method", parts.method.as_str());
        span.record("url.path", parts.uri.path());
        if let Some(query) = parts.uri.query() {
            span.record("url.query", query);
        }
        if let Some(scheme) = parts.uri.scheme() {
            span.record("url.scheme", scheme.as_str());
        }
        if let Some(request_id) = request_id.as_str() {
            span.record("http.request_id", &request_id);
        } else {
            span.record("http.request_id", request_id.as_bytes());
        }
        if let Some(client_ip) = client_ip {
            span.record("client.address", tracing::field::display(client_ip));
        }
        span.record("network.protocol.name", protocol.as_str());
        if let Protocol::Http = protocol {
            match parts.version {
                http::Version::HTTP_09 => span.record("network.protocol.version", 0.9),
                http::Version::HTTP_10 => span.record("network.protocol.version", 1.0),
                http::Version::HTTP_11 => span.record("network.protocol.version", 1.1),
                http::Version::HTTP_2 => span.record("network.protocol.version", 2),
                http::Version::HTTP_3 => span.record("network.protocol.version", 3),
                //Invalid version so just set 0
                _ => span.record("network.protocol.version", 0),
            };
        }

        drop(_entered);

        Self {
            span,
            info: RequestInfo {
                protocol,
                request_id,
                client_ip
            }
        }
    }
}

#[derive(Clone)]
///Tower layer
pub struct HttpRequestLayer {
    make_span: MakeSpan,
    inspect_headers: &'static [&'static http::HeaderName],
    extract_client_ip: ExtractClientIp,
}

impl HttpRequestLayer {
    #[inline]
    ///Creates new layer with provided span maker
    pub fn new(make_span: MakeSpan) -> Self {
        Self {
            make_span,
            inspect_headers: &[],
            extract_client_ip: default_client_ip
        }
    }

    #[inline]
    ///Specifies list of headers you want to inspect via `http.headers` attribute.
    ///
    ///By default none of the headers are inspected
    pub fn with_inspect_headers(mut self, inspect_headers: &'static [&'static http::HeaderName]) -> Self {
        self.inspect_headers = inspect_headers;
        self
    }

    ///Customizes client ip extraction method
    ///
    ///Default extracts none
    pub fn with_extract_client_ip(mut self, extract_client_ip: ExtractClientIp) -> Self {
        self.extract_client_ip = extract_client_ip;
        self
    }
}

impl<S> tower_layer::Layer<S> for HttpRequestLayer {
    type Service = HttpRequestService<S>;
    #[inline(always)]
    fn layer(&self, inner: S) -> Self::Service {
        HttpRequestService {
            layer: self.clone(),
            inner,
        }
    }
}

///Tower service to annotate requests with span
pub struct HttpRequestService<S> {
    layer: HttpRequestLayer,
    inner: S
}

impl<ReqBody, ResBody, S: tower_service::Service<http::Request<ReqBody>, Response = http::Response<ResBody>>> tower_service::Service<http::Request<ReqBody>> for HttpRequestService<S> where S::Error: std::error::Error {
    type Response = S::Response;
    type Error = S::Error;
    type Future = ResponseFut<S::Future>;

    #[inline(always)]
    fn poll_ready(&mut self, ctx: &mut task::Context<'_>) -> task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(ctx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let (parts, body) = req.into_parts();
        let RequestSpan { span, info } = RequestSpan::new((self.layer.make_span)(), self.layer.extract_client_ip, &parts);

        let _entered = span.enter();
        if !self.layer.inspect_headers.is_empty() {
            span.record("http.headers", tracing::field::debug(headers::InspectHeaders {
                header_list: self.layer.inspect_headers,
                headers: &parts.headers
            }));
        }
        let request_id = info.request_id.clone();
        let protocol = info.protocol;
        let mut req = http::Request::from_parts(parts, body);
        req.extensions_mut().insert(info);
        let inner = self.inner.call(req);

        drop(_entered);
        ResponseFut {
            inner,
            span,
            protocol,
            request_id
        }
    }
}

///Middleware's response future
pub struct ResponseFut<F> {
    inner: F,
    span: tracing::Span,
    protocol: Protocol,
    request_id: RequestId,
}

impl<ResBody, E: std::error::Error, F: Future<Output = Result<http::Response<ResBody>, E>>> Future for ResponseFut<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, ctx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let (fut, span, protocol, request_id) = unsafe {
            let this = self.get_unchecked_mut();
            (
                Pin::new_unchecked(&mut this.inner),
                &this.span,
                this.protocol,
                &this.request_id,
            )
        };
        let _entered = span.enter();
        match Future::poll(fut, ctx) {
            task::Poll::Ready(Ok(mut resp)) => {
                if let Ok(request_id) = http::HeaderValue::from_bytes(request_id.as_bytes()) {
                    resp.headers_mut().insert(REQUEST_ID, request_id);
                }
                let status = match protocol {
                    Protocol::Http => resp.status().as_u16(),
                    Protocol::Grpc => match resp.headers().get("grpc-status") {
                        Some(status) => grpc::parse_grpc_status(status.as_bytes()),
                        None => 2,
                    }
                };
                span.record("http.response.status_code", status);

                task::Poll::Ready(Ok(resp))
            }
            task::Poll::Ready(Err(error)) => {
                let status = match protocol {
                    Protocol::Http => 500u16,
                    Protocol::Grpc => 13,
                };
                span.record("http.response.status_code", status);
                span.record("error.type", core::any::type_name::<E>());
                span.record("error.message", tracing::field::display(&error));
                task::Poll::Ready(Err(error))
            },
            task::Poll::Pending => task::Poll::Pending
        }
    }
}

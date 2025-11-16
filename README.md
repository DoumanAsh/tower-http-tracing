# tower-http-tracing

[![Rust](https://github.com/DoumanAsh/tower-http-tracing/actions/workflows/rust.yml/badge.svg)](https://github.com/DoumanAsh/tower-http-tracing/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/tower-http-tracing.svg)](https://crates.io/crates/tower-http-tracing)
[![Documentation](https://docs.rs/tower-http-tracing/badge.svg)](https://docs.rs/crate/tower-http-tracing/)

Tower tracing middleware to annotate every HTTP request with tracing's span

## Example

Below is illustration of how to initialize request layer for passing into your service

```rust
use std::net::IpAddr;

use tower_http_tracing::{http, HttpRequestLayer};

#[derive(Clone)]
pub struct MyContext;

impl tower_http_tracing::LayerContext for MyContext {
    const INSPECT_HEADERS: &'static [&'static http::HeaderName] = &[&http::header::FORWARDED];

    //Logic to extract client ip has to be written by user
    //You can use utilities in separate crate to design this logic:
    //https://docs.rs/http-ip/latest/http_ip/
    fn extract_client_ip(&self, span: &tracing::Span, parts: &http::request::Parts) -> Option<IpAddr> {
        None
    }
}
tower_http_tracing::make_request_spanner!(make_my_request_span("my_request", tracing::Level::INFO));
let layer = HttpRequestLayer::new(make_my_request_span, MyContext);
//Use above layer in your service
```

## Features

- `opentelemetry` - Enables integration with opentelemetry to propagate context from requests and into responses
- `datadog` - Enables integration with specialized datadog tracing layer to propagate context from requests and into responses

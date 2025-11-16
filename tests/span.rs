use tower_http_tracing::{make_request_spanner, RequestSpan, Protocol, HttpRequestLayer, LayerContext};

use std::net::IpAddr;
use tower::{ServiceBuilder, ServiceExt};

make_request_spanner!(my_span("request", tracing::Level::INFO));
make_request_spanner!(my_span_with_custom_field("request", tracing::Level::INFO, service_name = "EXTRA", test = tracing::field::Empty));

#[derive(Copy, Clone)]
struct TestContext;

impl LayerContext for TestContext {
    const INSPECT_HEADERS: &'static [&'static http::HeaderName] = &[];

    fn extract_client_ip(&self, _: &tracing::Span, _: &http::request::Parts) -> Option<IpAddr> {
        "127.0.0.1".parse().ok()
    }
}

#[test]
#[tracing_test::traced_test]
fn should_generate_grpc_info() {
    let mut req = http::Request::new(());
    req.headers_mut().insert(http::header::CONTENT_TYPE, http::header::HeaderValue::from_static("application/grpc"));
    req.headers_mut().insert(tower_http_tracing::REQUEST_ID, http::HeaderValue::from_static("request-ID"));
    *req.uri_mut() = http::Uri::from_static("grpc://localhost/grpc.heatlh.v1.Health/Check");
    let (parts, ()) = req.into_parts();

    let span = my_span();
    let span = RequestSpan::new(&TestContext, span, &parts);
    assert_eq!(span.info.protocol, Protocol::Grpc);

    let _guard = span.span.enter();
    tracing::info!("LOG");
    drop(_guard);

    let expected_span = r#"should_generate_grpc_info:request{span.kind="server" http.request.method="GET" url.path="/grpc.heatlh.v1.Health/Check" url.scheme="grpc" http.request_id="request-ID" client.address=127.0.0.1 network.protocol.name="grpc""#;
    assert!(logs_contain(expected_span));
}

#[test]
#[tracing_test::traced_test]
fn should_generate_grpc_info_with_extra() {
    let mut req = http::Request::new(());
    req.headers_mut().insert(http::header::CONTENT_TYPE, http::header::HeaderValue::from_static("application/grpc"));
    req.headers_mut().insert(tower_http_tracing::REQUEST_ID, http::HeaderValue::from_static("request-ID"));
    *req.uri_mut() = http::Uri::from_static("grpc://localhost/grpc.heatlh.v1.Health/Check");
    let (parts, ()) = req.into_parts();

    let span = my_span_with_custom_field();
    let span = RequestSpan::new(&TestContext, span, &parts);
    assert_eq!(span.info.protocol, Protocol::Grpc);

    let _guard = span.span.enter();
    tracing::info!("LOG");
    drop(_guard);

    let expected_span = r#"span.kind="server" service_name="EXTRA" http.request.method="GET" url.path="/grpc.heatlh.v1.Health/Check" url.scheme="grpc" http.request_id="request-ID" client.address=127.0.0.1 network.protocol.name="grpc"#;
    assert!(logs_contain(expected_span));
}

#[test]
#[tracing_test::traced_test]
fn should_generate_http_info() {
    let mut req = http::Request::new(());
    req.headers_mut().insert(http::header::CONTENT_TYPE, http::header::HeaderValue::from_static("application/json"));
    req.headers_mut().insert(tower_http_tracing::REQUEST_ID, http::HeaderValue::from_static("request-ID"));
    *req.uri_mut() = http::Uri::from_static("http://localhost/index.html");
    let (parts, ()) = req.into_parts();

    let span = my_span();
    let span = RequestSpan::new(&TestContext, span, &parts);
    assert_eq!(span.info.protocol, Protocol::Http);

    let _guard = span.span.enter();
    tracing::info!("LOG");
    drop(_guard);

    let expected_span = r#"should_generate_http_info:request{span.kind="server" http.request.method="GET" url.path="/index.html" url.scheme="http" http.request_id="request-ID" client.address=127.0.0.1 network.protocol.name="http" network.protocol.version=1.1"#;
    assert!(logs_contain(expected_span));
}

#[tokio::test]
async fn should_complete_successful_request_span() {
    const REQUEST_ID_VALUE: &str = "successful-id";
    let layer = HttpRequestLayer::new(my_span, TestContext);
    let service = ServiceBuilder::new().layer(layer).service_fn(|_: http::Request<()>| async move {
        Ok::<_, core::convert::Infallible>(http::Response::new(()))
    });

    let mut request = http::Request::new(());
    request.headers_mut().insert(tower_http_tracing::REQUEST_ID, http::HeaderValue::from_static(REQUEST_ID_VALUE));
    let res = service.oneshot(request).await.unwrap();
    let request_id = res.headers().get(tower_http_tracing::REQUEST_ID).unwrap();
    assert_eq!(request_id.to_str().expect("request id must be valid string"), REQUEST_ID_VALUE);
}

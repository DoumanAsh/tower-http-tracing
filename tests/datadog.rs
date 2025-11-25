use tracing_datadog::context::{Strategy, DatadogContext};

#[test]
fn should_ensure_datadog_context_propagation() {
    let mut headers = http::HeaderMap::new();

    let context = DatadogContext {
        trace_id: 0xfff00011110002222,
        parent_id: 0x333000444000555,
    };

    let empty = tower_http_tracing::datadog::Propagation::extract(&headers);
    assert_eq!(empty.trace_id, 0);
    assert_eq!(empty.parent_id, 0);

    tower_http_tracing::datadog::Propagation::inject(&mut headers, context);

    let not_empty = tower_http_tracing::datadog::Propagation::extract(&headers);
    assert_eq!(not_empty.trace_id, context.trace_id);
    assert_eq!(not_empty.parent_id, context.parent_id);

    let header_value = headers.get(tower_http_tracing::datadog::W3C_TRACEPARENT_NAME).map(|value| value.to_str().expect("should be string")).unwrap();
    assert_eq!(header_value, "00-000000000000000fff00011110002222-0333000444000555-01");
}

#[test]
fn should_ensure_datadog_trace_not_sampled() {
    let mut headers = http::HeaderMap::new();
    headers.insert(tower_http_tracing::datadog::W3C_TRACEPARENT_NAME, "00-000000000000000fff00011110002222-0333000444000555-10".parse().unwrap());
    let context = tower_http_tracing::datadog::Propagation::extract(&headers);
    assert_eq!(context.trace_id, 0);
    assert_eq!(context.parent_id, 0);
}

#[test]
fn should_ensure_datadog_context_with_invalid_version_rejected() {
    let mut headers = http::HeaderMap::new();
    headers.insert(tower_http_tracing::datadog::W3C_TRACEPARENT_NAME, "01-000000000000000fff00011110002222-0333000444000555-01".parse().unwrap());
    let context = tower_http_tracing::datadog::Propagation::extract(&headers);
    assert_eq!(context.trace_id, 0);
    assert_eq!(context.parent_id, 0);
}

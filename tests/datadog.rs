use tracing_datadog::context::{Strategy, DatadogContext};

#[test]
fn should_ensure_tracing_context_propagation() {
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

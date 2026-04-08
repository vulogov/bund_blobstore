// tests/commongrokintegration-test.rs
use bund_blobstore::common::GrokLogParser;

#[test]
fn test_parser_creation() {
    let parser = GrokLogParser::new("test_app");
    let result = parser.process_log_line("2024-01-15T10:30:45Z INFO [main] test: message");
    assert!(result.is_ok());
}

#[test]
fn test_parse_common_log() {
    let parser = GrokLogParser::new("test");

    let log_line = "2024-01-15T10:30:45Z INFO [main] user_login: User logged in successfully";
    let record = parser.process_log_line(log_line).unwrap();

    assert_eq!(record.key, "user_login");
    assert_eq!(record.source, "test");
    assert!(record.metadata.contains_key("level"));
    assert_eq!(record.metadata.get("level").unwrap(), "INFO");
}

#[test]
fn test_parse_bund_telemetry() {
    let parser = GrokLogParser::new("bund");

    let log_line =
        "2024-01-15T10:30:45Z|vector_db|similarity_search|duration=87ms|top_k=10,threshold=0.8";
    let record = parser.process_log_line(log_line).unwrap();

    assert_eq!(record.key, "similarity_search");
    assert_eq!(record.source, "vector_db");
}

#[test]
fn test_parse_vector_operation() {
    let parser = GrokLogParser::new("vector");

    let log_line = "VECTOR|search|dim=1536|time=87ms|top_k=10,threshold=0.8";
    let record = parser.process_log_line(log_line).unwrap();

    assert_eq!(record.key, "search");
    assert!(record.metadata.contains_key("dimension"));
    assert!(record.metadata.contains_key("time_ms"));
    assert_eq!(record.metadata.get("dimension").unwrap(), "1536");
}

#[test]
fn test_parse_search_query() {
    let parser = GrokLogParser::new("search");

    let log_line = "SEARCH|users|query=john doe|results=42|time=125ms";
    let record = parser.process_log_line(log_line).unwrap();

    assert_eq!(record.key, "users");
    assert!(record.metadata.contains_key("query"));
    assert_eq!(record.metadata.get("query").unwrap(), "john doe");
}

#[test]
fn test_custom_pattern_addition() {
    let parser = GrokLogParser::new("custom");

    parser
        .add_pattern(
            "custom_metric",
            r"METRIC\|(?P<key>\w+)\|(?P<value>\d+(?:\.\d+)?)\|(?P<metadata>.+)",
        )
        .unwrap();

    let log_line = "METRIC|cpu_usage|42.5|unit=percent,core=0";
    let record = parser
        .process_with_pattern("custom_metric", log_line)
        .unwrap();

    assert_eq!(record.key, "cpu_usage");
    assert!(matches!(
        record.value,
        bund_blobstore::timeline::TelemetryValue::Float(42.5)
    ));

    let has_unit = record.metadata.values().any(|v| v.contains("percent"));
    assert!(has_unit, "Metadata should contain unit information");
}

#[test]
fn test_batch_processing() {
    let parser = GrokLogParser::new("batch");

    let log_lines = vec![
        "2024-01-15T10:30:45Z INFO [main] event1: First event".to_string(),
        "2024-01-15T10:30:46Z ERROR [worker] event2: Second event".to_string(),
        "2024-01-15T10:30:47Z DEBUG [cache] event3: Third event".to_string(),
    ];

    let results = parser.process_batch(log_lines);
    assert_eq!(results.len(), 3);

    let success_count = results.iter().filter(|r| r.is_ok()).count();
    assert_eq!(success_count, 3);
}

#[test]
fn test_no_match() {
    let parser = GrokLogParser::new("test");

    let log_line = "This line doesn't match any pattern";
    let result = parser.process_log_line(log_line);
    assert!(result.is_err());
}

#[test]
fn test_metadata_extraction() {
    let parser = GrokLogParser::new("test");

    let log_line = "2024-01-15T10:30:45Z ERROR [worker] db_error: Connection failed";
    let record = parser.process_log_line(log_line).unwrap();

    assert!(record.metadata.contains_key("level"));
    assert!(record.metadata.contains_key("thread"));
    assert_eq!(record.metadata.get("level").unwrap(), "ERROR");
    assert_eq!(record.metadata.get("thread").unwrap(), "worker");
}

// Test JSON parsing with timestamp extraction
#[test]
fn test_json_parsing_with_timestamp() {
    let parser = GrokLogParser::new("json_test");

    // Add pattern for JSON-like structured log with timestamp
    parser.add_pattern("json_structured",
        r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) level=(?P<level>\w+) key=(?P<key>\w+) value=(?P<value>\d+)"
    ).unwrap();

    let log_line = "timestamp=2024-01-15T10:30:45Z level=INFO key=api_request value=200";
    let record = parser
        .process_with_pattern("json_structured", log_line)
        .unwrap();

    // Test key extraction
    assert_eq!(record.key, "api_request");

    // Test value extraction
    assert!(matches!(
        record.value,
        bund_blobstore::timeline::TelemetryValue::Int(200)
    ));

    // Test timestamp extraction
    let expected_timestamp = 1705314645; // 2024-01-15T10:30:45Z as Unix timestamp
    assert_eq!(record.timestamp_seconds, expected_timestamp);

    // Test metadata extraction
    assert!(record.metadata.contains_key("level"));
    assert_eq!(record.metadata.get("level").unwrap(), "INFO");
}

// Test JSON with different timestamp formats
#[test]
fn test_json_timestamp_formats() {
    let parser = GrokLogParser::new("timestamp_test");

    // Test ISO8601 format
    parser
        .add_pattern(
            "iso_timestamp",
            r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) key=(?P<key>\w+)",
        )
        .unwrap();

    let log_line = "timestamp=2024-01-15T10:30:45Z key=test_key";
    let record = parser
        .process_with_pattern("iso_timestamp", log_line)
        .unwrap();
    assert_eq!(record.timestamp_seconds, 1705314645);

    // Test Unix timestamp format
    parser
        .add_pattern(
            "unix_timestamp",
            r"timestamp=(?P<timestamp>\d+) key=(?P<key>\w+)",
        )
        .unwrap();

    let log_line = "timestamp=1705314645 key=test_key";
    let record = parser
        .process_with_pattern("unix_timestamp", log_line)
        .unwrap();
    assert_eq!(record.timestamp_seconds, 1705314645);

    // Test custom datetime format
    parser
        .add_pattern(
            "custom_timestamp",
            r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}) key=(?P<key>\w+)",
        )
        .unwrap();

    let log_line = "timestamp=2024-01-15 10:30:45 key=test_key";
    let record = parser
        .process_with_pattern("custom_timestamp", log_line)
        .unwrap();
    assert_eq!(record.timestamp_seconds, 1705314645);
}

// Test complete JSON log parsing with all fields
#[test]
fn test_complete_json_log_parsing() {
    let parser = GrokLogParser::new("complete_json");

    // Pattern for complete JSON-like structured log
    parser.add_pattern("complete_json",
        r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) level=(?P<level>\w+) source=(?P<source>\w+) key=(?P<key>\w+) value=(?P<value>\d+) duration_ms=(?P<duration>\d+)"
    ).unwrap();

    let log_line = "timestamp=2024-01-15T10:30:45Z level=INFO source=api key=user_login value=1 duration_ms=125";
    let record = parser
        .process_with_pattern("complete_json", log_line)
        .unwrap();

    // Test all fields
    assert_eq!(record.key, "user_login");
    assert_eq!(record.source, "api");
    assert_eq!(record.timestamp_seconds, 1705314645);
    assert!(matches!(
        record.value,
        bund_blobstore::timeline::TelemetryValue::Int(1)
    ));

    // Test metadata
    assert!(record.metadata.contains_key("level"));
    assert_eq!(record.metadata.get("level").unwrap(), "INFO");
    assert!(record.metadata.contains_key("duration"));
    assert_eq!(record.metadata.get("duration").unwrap(), "125");
}

// Test JSON array and object handling
#[test]
fn test_json_complex_types() {
    let parser = GrokLogParser::new("complex_json");

    // Test array in JSON
    parser.add_pattern("json_array",
        r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) key=(?P<key>\w+) tags=(?P<tags>\[.*\])"
    ).unwrap();

    let log_line =
        r#"timestamp=2024-01-15T10:30:45Z key=search tags=["vector", "semantic", "hybrid"]"#;
    let record = parser.process_with_pattern("json_array", log_line).unwrap();

    assert_eq!(record.key, "search");
    assert_eq!(record.timestamp_seconds, 1705314645);

    // Test object in JSON
    parser.add_pattern("json_object",
        r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) key=(?P<key>\w+) metadata=(?P<metadata>\{.*\})"
    ).unwrap();

    let log_line =
        r#"timestamp=2024-01-15T10:30:45Z key=query metadata={"score": 0.95, "chunks": 10}"#;
    let record = parser
        .process_with_pattern("json_object", log_line)
        .unwrap();

    assert_eq!(record.key, "query");
    assert_eq!(record.timestamp_seconds, 1705314645);
}

// Test that timestamp extraction handles missing timestamps gracefully
#[test]
fn test_missing_timestamp() {
    let parser = GrokLogParser::new("no_timestamp");

    parser
        .add_pattern(
            "no_timestamp_pattern",
            r"key=(?P<key>\w+) value=(?P<value>\d+)",
        )
        .unwrap();

    let log_line = "key=test_key value=42";
    let record = parser
        .process_with_pattern("no_timestamp_pattern", log_line)
        .unwrap();

    assert_eq!(record.key, "test_key");
    assert!(matches!(
        record.value,
        bund_blobstore::timeline::TelemetryValue::Int(42)
    ));

    // Timestamp should be set to current time (within last 5 seconds)
    let current_time = chrono::Utc::now().timestamp();
    assert!(record.timestamp_seconds <= current_time);
    assert!(record.timestamp_seconds >= current_time - 5);
}

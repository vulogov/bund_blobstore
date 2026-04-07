```markdown
# GROK Integration Documentation

## Overview

The `GrokLogParser` module provides powerful log parsing capabilities for converting unstructured log messages into structured `TelemetryRecord` objects. It uses regex-based patterns (compatible with Grok patterns) to extract meaningful telemetry data from various log formats.

## Features

- **Multi-format log parsing** - Supports common log formats, JSON, key-value pairs, and custom patterns
- **Automatic pattern matching** - Tries multiple patterns to find the best match for each log line
- **Timestamp extraction** - Handles multiple timestamp formats (ISO8601, Unix, custom)
- **Value parsing** - Automatically detects and converts integers, floats, booleans, and JSON
- **Metadata extraction** - Captures additional fields as metadata
- **Batch processing** - Process multiple log lines efficiently
- **Custom patterns** - Add your own patterns for domain-specific logs

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["grok-integration"] }
```

## Quick Start

```rust
use bund_blobstore::common::GrokLogParser;

// Create a parser with default patterns
let parser = GrokLogParser::new("my_app");

// Parse a log line
let log_line = "2024-01-15T10:30:45Z INFO [main] user_login: User logged in";
let record = parser.process_log_line(log_line)?;

println!("Key: {}", record.key);
println!("Source: {}", record.source);
println!("Timestamp: {}", record.timestamp_seconds);
println!("Value: {:?}", record.value);
println!("Metadata: {:?}", record.metadata);
```

## Default Patterns

The parser comes pre-configured with these patterns:

### 1. Common Log Format
```
2024-01-15T10:30:45Z INFO [main] user_login: User logged in successfully
```
Extracts: `timestamp`, `level`, `thread`, `key`, `message`

### 2. Bund Telemetry Format
```
2024-01-15T10:30:45Z|vector_db|similarity_search|duration=87ms|top_k=10,threshold=0.8
```
Extracts: `timestamp`, `source`, `key`, `value`, `metadata`

### 3. Vector Operation Format
```
VECTOR|search|dim=1536|time=87ms|top_k=10,threshold=0.8
```
Extracts: `operation`, `dimension`, `time_ms`, `metadata`

### 4. Search Query Format
```
SEARCH|users|query=john doe|results=42|time=125ms
```
Extracts: `index`, `query`, `results`, `time_ms`

### 5. Graph Operation Format
```
GRAPH|shortest_path|nodes=100|edges=250|time=45ms
```
Extracts: `operation`, `nodes`, `edges`, `time_ms`

## Usage Examples

### Basic Log Parsing

```rust
let parser = GrokLogParser::new("application");

// Parse standard log
let log = "2024-01-15T10:30:45Z ERROR [worker] database: Connection failed";
let record = parser.process_log_line(log)?;

assert_eq!(record.key, "database");
assert_eq!(record.source, "application");
assert!(record.metadata.contains_key("level"));
assert_eq!(record.metadata.get("level").unwrap(), "ERROR");
```

### Parsing Vector Telemetry

```rust
let parser = GrokLogParser::new("vector_db");

let log = "VECTOR|index|dim=1536|time=1234ms|vectors=10000";
let record = parser.process_log_line(log)?;

assert_eq!(record.key, "index");
assert_eq!(record.metadata.get("dimension").unwrap(), "1536");
assert_eq!(record.metadata.get("time_ms").unwrap(), "1234");
```

### Parsing RAG Operations

```rust
let parser = GrokLogParser::new("rag_system");

// Add custom pattern for RAG operations
parser.add_pattern("rag_query", 
    r"RAG\|(?P<operation>\w+)\|query=(?P<query>.*)\|chunks=(?P<chunks>\d+)\|time=(?P<time_ms>\d+)ms\|score=(?P<score>\d+\.\d+)"
)?;

let log = "RAG|search|query=vector database|chunks=10|time=245ms|score=0.89";
let record = parser.process_with_pattern("rag_query", log)?;

assert_eq!(record.key, "search");
assert_eq!(record.metadata.get("chunks").unwrap(), "10");
assert_eq!(record.metadata.get("score").unwrap(), "0.89");
```

### Parsing JSON Logs

```rust
let parser = GrokLogParser::new("api");

// Add JSON pattern
parser.add_pattern("json_log",
    r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) level=(?P<level>\w+) key=(?P<key>\w+) value=(?P<value>\d+)"
)?;

let log = "timestamp=2024-01-15T10:30:45Z level=INFO key=api_request value=200";
let record = parser.process_with_pattern("json_log", log)?;

assert_eq!(record.key, "api_request");
assert_eq!(record.timestamp_seconds, 1705314645);
```

### Batch Processing

```rust
let parser = GrokLogParser::new("batch_processor");

let log_lines = vec![
    "2024-01-15T10:30:45Z INFO [main] event1: Started".to_string(),
    "2024-01-15T10:30:46Z ERROR [worker] event2: Failed".to_string(),
    "2024-01-15T10:30:47Z DEBUG [cache] event3: Cache hit".to_string(),
];

let results = parser.process_batch(log_lines);
for result in results {
    if let Ok(record) = result {
        println!("Processed: {}", record.key);
    }
}
```

### Custom Pattern Creation

```rust
let parser = GrokLogParser::new("custom_system");

// Pattern syntax: (?P<field_name>regex)
parser.add_pattern("custom_metric", 
    r"METRIC\|(?P<key>\w+)\|(?P<value>\d+(?:\.\d+)?)\|(?P<metadata>.+)"
)?;

let log = "METRIC|cpu_usage|42.5|unit=percent,core=0";
let record = parser.process_with_pattern("custom_metric", log)?;

assert_eq!(record.key, "cpu_usage");
assert!(matches!(record.value, TelemetryValue::Float(42.5)));
```

### Timestamp Extraction

The parser automatically handles multiple timestamp formats:

```rust
let parser = GrokLogParser::new("timestamp_test");

// ISO8601 format
parser.add_pattern("iso", r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")?;
let log = "timestamp=2024-01-15T10:30:45Z";
let record = parser.process_with_pattern("iso", log)?;
assert_eq!(record.timestamp_seconds, 1705314645);

// Unix timestamp
parser.add_pattern("unix", r"timestamp=(?P<timestamp>\d+)")?;
let log = "timestamp=1705314645";
let record = parser.process_with_pattern("unix", log)?;
assert_eq!(record.timestamp_seconds, 1705314645);

// Custom format
parser.add_pattern("custom", r"timestamp=(?P<timestamp>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2})")?;
let log = "timestamp=2024-01-15 10:30:45";
let record = parser.process_with_pattern("custom", log)?;
assert_eq!(record.timestamp_seconds, 1705314645);
```

### Metadata Extraction

```rust
let parser = GrokLogParser::new("metadata_test");

parser.add_pattern("rich_log",
    r"level=(?P<level>\w+) key=(?P<key>\w+) user=(?P<user>\w+) duration=(?P<duration>\d+)ms"
)?;

let log = "level=INFO key=api_call user=john duration=125ms";
let record = parser.process_with_pattern("rich_log", log)?;

// Access metadata fields
assert_eq!(record.metadata.get("level").unwrap(), "INFO");
assert_eq!(record.metadata.get("user").unwrap(), "john");
assert_eq!(record.metadata.get("duration").unwrap(), "125");
```

### File Processing

```rust
use bund_blobstore::common::convert_log_file_to_telemetry;

let parser = GrokLogParser::new("log_processor");
let records = convert_log_file_to_telemetry(&parser, std::path::Path::new("app.log"))?;

println!("Processed {} telemetry records", records.len());
```

## Pattern Syntax Guide

### Capture Groups
Use `(?P<field_name>regex)` to capture named fields:
- `(?P<timestamp>\d+)` - Captures digits as timestamp
- `(?P<key>\w+)` - Captures word characters as key
- `(?P<value>\d+(?:\.\d+)?)` - Captures integers or floats as value

### Common Regex Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `\d+` | One or more digits | 123, 4567 |
| `\w+` | One or more word characters | hello, user123 |
| `\s+` | One or more whitespace | space, tab |
| `[^|]+` | One or more non-pipe characters | value, data |
| `.*` | Any characters (greedy) | everything |
| `.*?` | Any characters (non-greedy) | minimal match |

### Example Patterns

```rust
// Key-value pairs
r"(?P<key>\w+)=(?P<value>[^,]+)"

// CSV format
r"(?P<timestamp>[^,]+),(?P<level>[^,]+),(?P<key>[^,]+),(?P<message>.*)"

// Pipe-separated
r"(?P<timestamp>[^|]+)\|(?P<level>[^|]+)\|(?P<key>[^|]+)\|(?P<value>[^|]+)"

// Bracket-delimited
r"\[(?P<timestamp>[^\]]+)\] \[(?P<level>[^\]]+)\] (?P<key>\w+): (?P<message>.*)"
```

## Integration with Telemetry Timeline

```rust
use bund_blobstore::timeline::TelemetryTimeline;

let parser = GrokLogParser::new("production");
let timeline = TelemetryTimeline::new("./telemetry_data")?;

// Parse and store logs
let logs = vec![
    "2024-01-15T10:30:45Z INFO [api] request: GET /users",
    "2024-01-15T10:30:46Z INFO [db] query: SELECT * FROM users",
];

for log in logs {
    if let Ok(record) = parser.process_log_line(log) {
        timeline.insert(
            &record.key,
            record.timestamp_seconds,
            &record.value.to_string(),
            Some(record.metadata),
        )?;
    }
}

// Query telemetry data
let start = 1705314600;
let end = 1705314700;
let results = timeline.query_range("request", start, end)?;
```

## Error Handling

```rust
match parser.process_log_line(log_line) {
    Ok(record) => {
        // Successfully parsed
        println!("Record: {:?}", record);
    }
    Err(e) => {
        // Handle parsing error
        eprintln!("Failed to parse log: {}", e);
    }
}
```

## Performance Considerations

1. **Batch Processing**: Use `process_batch()` for multiple logs
2. **Pattern Order**: Patterns are tried in order; put most common patterns first
3. **Regex Compilation**: Patterns are compiled once and reused
4. **Concurrent Access**: The parser uses `Arc<RwLock>` for thread-safe pattern access

## Advanced Examples

### Multi-line Log Processing

```rust
let parser = GrokLogParser::new("multi_line");
let mut records = Vec::new();

// Process log file with continuation lines
let mut current_log = String::new();
for line in log_lines {
    if line.starts_with("TIMESTAMP") {
        if !current_log.is_empty() {
            if let Ok(record) = parser.process_log_line(&current_log) {
                records.push(record);
            }
            current_log.clear();
        }
    }
    current_log.push_str(&line);
}
```

### Dynamic Pattern Selection

```rust
let parser = GrokLogParser::new("dynamic");

// Try different patterns based on log prefix
let record = if log_line.starts_with("VECTOR") {
    parser.process_with_pattern("vector_operation", log_line)
} else if log_line.starts_with("SEARCH") {
    parser.process_with_pattern("search_query", log_line)
} else {
    parser.process_log_line(log_line) // Auto-detect
}?;
```

## Troubleshooting

### Common Issues

1. **No pattern matches**: Add custom pattern or check regex syntax
2. **Wrong field extraction**: Verify capture group names in pattern
3. **Timestamp parsing fails**: Add timestamp field to pattern or check format
4. **Value not parsed**: Ensure value matches expected type (int, float, bool)

### Debugging

```rust
// Enable debug logging
tracing_subscriber::fmt()
    .with_max_level(tracing::Level::DEBUG)
    .init();

// The parser will log which pattern matched
parser.process_log_line(log_line)?;
// Output: DEBUG Matched pattern: common_log for log: ...
```

## API Reference

### `GrokLogParser`

| Method | Description |
|--------|-------------|
| `new(source: impl Into<String>) -> Self` | Creates new parser with default patterns |
| `add_pattern(&self, name: &str, pattern: &str) -> Result<()>` | Adds custom pattern |
| `process_log_line(&self, line: &str) -> Result<TelemetryRecord>` | Parses single log line |
| `process_batch(&self, lines: Vec<String>) -> Vec<Result<TelemetryRecord>>` | Parses multiple lines |
| `process_with_pattern(&self, name: &str, line: &str) -> Result<TelemetryRecord>` | Uses specific pattern |
| `parse_auto(&self, line: &str) -> Result<HashMap<String, String>>` | Returns raw parsed fields |
| `parse_with_pattern(&self, name: &str, line: &str) -> Result<HashMap<String, String>>` | Raw parse with pattern |

### Helper Functions

| Function | Description |
|----------|-------------|
| `convert_log_file_to_telemetry(parser: &GrokLogParser, path: &Path) -> Result<Vec<TelemetryRecord>>` | Processes entire log file |

## Best Practices

1. **Name capture groups clearly** - Use descriptive names like `timestamp`, `user_id`, `duration_ms`
2. **Be specific with regex** - Avoid greedy `.*` when possible
3. **Test patterns thoroughly** - Use the test suite as reference
4. **Cache patterns** - Patterns are compiled once and reused
5. **Handle errors gracefully** - Always match on `Result` types

## See Also


```

This documentation provides comprehensive coverage of the Grok integration module with practical examples and best practices for integrating it into your applications.

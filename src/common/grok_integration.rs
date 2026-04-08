// src/common/grok_integration.rs
use crate::timeline::{TelemetryRecord, TelemetryValue};
use chrono::{DateTime, NaiveDateTime, Utc};
use parking_lot::RwLock;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

pub type Result<T> = std::result::Result<T, String>;

/// Pattern-based log parser (compatible with Grok patterns)
pub struct GrokLogParser {
    patterns: Arc<RwLock<HashMap<String, Regex>>>,
    default_source: String,
}

impl GrokLogParser {
    /// Create a new parser with default patterns
    pub fn new(default_source: impl Into<String>) -> Self {
        let parser = Self {
            patterns: Arc::new(RwLock::new(HashMap::new())),
            default_source: default_source.into(),
        };

        parser.initialize_default_patterns();
        parser
    }

    /// Initialize common log patterns
    // Update the initialize_default_patterns method in grok_integration.rs
    fn initialize_default_patterns(&self) {
        let patterns: Vec<(&str, &str)> = vec![
            (
                "common_log",
                r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z) (?P<level>\w+) \[(?P<thread>\w+)\] (?P<key>\w+): (?P<message>.*)",
            ),
            (
                "bund_telemetry",
                r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)\|(?P<source>\w+)\|(?P<key>\w+)\|(?P<value>[^|]+)\|(?P<metadata>.*)",
            ),
            (
                "vector_operation",
                r"VECTOR\|(?P<operation>\w+)\|dim=(?P<dimension>\d+)\|time=(?P<time_ms>\d+)ms\|(?P<metadata>.*)",
            ),
            (
                "search_query",
                r"SEARCH\|(?P<index>\w+)\|query=(?P<query>.*)\|results=(?P<results>\d+)\|time=(?P<time_ms>\d+)ms",
            ),
            (
                "graph_operation",
                r"GRAPH\|(?P<operation>\w+)\|nodes=(?P<nodes>\d+)\|edges=(?P<edges>\d+)\|time=(?P<time_ms>\d+)ms",
            ),
            ("json_log", r"{(?P<json>.*)}"),
        ];

        for (name, pattern) in patterns {
            let _ = self.add_pattern(name, pattern);
        }
    }

    /// Add a custom pattern
    pub fn add_pattern(&self, name: &str, pattern: &str) -> Result<()> {
        let regex = Regex::new(pattern)
            .map_err(|e| format!("Failed to compile pattern '{}': {}", name, e))?;

        let mut patterns = self.patterns.write();
        patterns.insert(name.to_string(), regex);

        debug!("Added pattern: {}", name);
        Ok(())
    }

    /// Parse a log line using a specific pattern
    pub fn parse_with_pattern(
        &self,
        pattern_name: &str,
        log_line: &str,
    ) -> Result<HashMap<String, String>> {
        let patterns = self.patterns.read();
        let regex = patterns
            .get(pattern_name)
            .ok_or_else(|| format!("Pattern not found: {}", pattern_name))?;

        match regex.captures(log_line) {
            Some(caps) => {
                let mut result = HashMap::new();
                for name in regex.capture_names().filter_map(|n| n) {
                    if let Some(value) = caps.name(name) {
                        result.insert(name.to_string(), value.as_str().to_string());
                    }
                }
                Ok(result)
            }
            None => Err(format!(
                "Failed to parse log line with pattern '{}'",
                pattern_name
            )),
        }
    }

    /// Parse a log line by trying all available patterns
    pub fn parse_auto(&self, log_line: &str) -> Result<HashMap<String, String>> {
        let patterns = self.patterns.read();

        for (name, regex) in patterns.iter() {
            if let Some(caps) = regex.captures(log_line) {
                let mut result = HashMap::new();
                for capture_name in regex.capture_names().filter_map(|n| n) {
                    if let Some(value) = caps.name(capture_name) {
                        result.insert(capture_name.to_string(), value.as_str().to_string());
                    }
                }
                debug!("Matched pattern: {} for log: {}", name, log_line);
                return Ok(result);
            }
        }

        Err("No pattern matched the log line".to_string())
    }

    /// Generate a unique ID (simple implementation)
    fn generate_id(&self) -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("telemetry_{}", timestamp)
    }

    /// Convert a parsed log line to a TelemetryRecord
    pub fn to_telemetry_record(
        &self,
        parsed: HashMap<String, String>,
        raw_log: &str,
    ) -> Result<TelemetryRecord> {
        let timestamp = self.extract_timestamp(&parsed)?;
        let key = self.extract_key(&parsed, raw_log)?;
        let source = self.extract_source(&parsed)?;
        let value = self.extract_value(&parsed, raw_log)?;
        let metadata = self.extract_metadata(&parsed);

        Ok(TelemetryRecord {
            id: self.generate_id(),
            timestamp_seconds: timestamp,
            key,
            source,
            value,
            metadata,
            is_primary: false,
            primary_id: None,
            secondary_ids: Vec::new(),
        })
    }

    /// Extract timestamp from parsed fields
    fn extract_timestamp(&self, parsed: &HashMap<String, String>) -> Result<i64> {
        if let Some(ts_str) = parsed.get("timestamp") {
            // Try ISO8601 format
            if let Ok(dt) = DateTime::parse_from_rfc3339(ts_str) {
                return Ok(dt.timestamp());
            }

            // Try Unix timestamp
            if let Ok(ts) = ts_str.parse::<i64>() {
                return Ok(ts);
            }

            // Try custom format using NaiveDateTime
            if let Ok(naive_dt) = NaiveDateTime::parse_from_str(ts_str, "%Y-%m-%d %H:%M:%S") {
                return Ok(naive_dt.and_utc().timestamp());
            }
        }

        // Default to current time if no timestamp found
        warn!("No timestamp found in log, using current time");
        Ok(Utc::now().timestamp())
    }

    /// Extract key from parsed fields
    fn extract_key(&self, parsed: &HashMap<String, String>, raw_log: &str) -> Result<String> {
        if let Some(key) = parsed.get("key") {
            return Ok(key.clone());
        }

        if let Some(operation) = parsed.get("operation") {
            return Ok(operation.clone());
        }

        if let Some(index) = parsed.get("index") {
            return Ok(index.clone());
        }

        if let Some(level) = parsed.get("level") {
            return Ok(format!("log.{}", level.to_lowercase()));
        }

        Ok(format!("log_{}", raw_log.len()))
    }

    /// Extract source from parsed fields
    fn extract_source(&self, parsed: &HashMap<String, String>) -> Result<String> {
        if let Some(source) = parsed.get("source") {
            return Ok(source.clone());
        }

        if let Some(host) = parsed.get("host") {
            return Ok(host.clone());
        }

        Ok(self.default_source.clone())
    }

    /// Extract value from parsed fields
    fn extract_value(
        &self,
        parsed: &HashMap<String, String>,
        raw_log: &str,
    ) -> Result<TelemetryValue> {
        if let Some(value) = parsed.get("value") {
            return self.parse_value(value);
        }

        if let Some(message) = parsed.get("message") {
            return self.parse_value(message);
        }

        if let Some(results) = parsed.get("results") {
            if let Ok(count) = results.parse::<i64>() {
                return Ok(TelemetryValue::Int(count));
            }
        }

        if let Some(nodes) = parsed.get("nodes") {
            if let Ok(count) = nodes.parse::<i64>() {
                return Ok(TelemetryValue::Int(count));
            }
        }

        Ok(TelemetryValue::String(raw_log.to_string()))
    }

    /// Parse a string value into TelemetryValue
    fn parse_value(&self, value_str: &str) -> Result<TelemetryValue> {
        // Try to parse as integer
        if let Ok(int_val) = value_str.parse::<i64>() {
            return Ok(TelemetryValue::Int(int_val));
        }

        // Try to parse as float
        if let Ok(float_val) = value_str.parse::<f64>() {
            return Ok(TelemetryValue::Float(float_val));
        }

        // Try to parse as boolean
        match value_str.to_lowercase().as_str() {
            "true" => return Ok(TelemetryValue::Bool(true)),
            "false" => return Ok(TelemetryValue::Bool(false)),
            "null" => return Ok(TelemetryValue::Null),
            _ => {}
        }

        // Try to parse as JSON
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(value_str) {
            return Ok(self.convert_json_to_telemetry_value(json_val));
        }

        Ok(TelemetryValue::String(value_str.to_string()))
    }

    /// Convert JSON value to TelemetryValue
    fn convert_json_to_telemetry_value(&self, json_val: serde_json::Value) -> TelemetryValue {
        match json_val {
            serde_json::Value::Null => TelemetryValue::Null,
            serde_json::Value::Bool(b) => TelemetryValue::Bool(b),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    TelemetryValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    TelemetryValue::Float(f)
                } else {
                    TelemetryValue::String(n.to_string())
                }
            }
            serde_json::Value::String(s) => TelemetryValue::String(s),
            serde_json::Value::Array(arr) => {
                // Convert to a simple string representation for now
                TelemetryValue::String(format!("{:?}", arr))
            }
            serde_json::Value::Object(obj) => {
                // Convert to a simple string representation for now
                TelemetryValue::String(format!("{:?}", obj))
            }
        }
    }
    fn parse_metadata_string(&self, metadata_str: &str) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        // Parse key=value pairs separated by commas
        for pair in metadata_str.split(',') {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() == 2 {
                metadata.insert(parts[0].trim().to_string(), parts[1].trim().to_string());
            }
        }

        metadata
    }
    /// Extract metadata from parsed fields
    fn extract_metadata(&self, parsed: &HashMap<String, String>) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        // If there's a dedicated metadata field, parse it
        if let Some(metadata_str) = parsed.get("metadata") {
            let parsed_metadata = self.parse_metadata_string(metadata_str);
            metadata.extend(parsed_metadata);
        }

        // Known metadata fields to include
        let metadata_fields = vec![
            "level",
            "thread",
            "host",
            "pid",
            "module",
            "method",
            "path",
            "operation",
            "dimension",
            "time_ms",
            "query",
            "index",
            "nodes",
            "edges",
            "results",
            "duration_ms",
            "rows",
        ];

        for field in &metadata_fields {
            if let Some(value) = parsed.get(*field) {
                metadata.insert(field.to_string(), value.clone());
            }
        }

        // Add any remaining fields as metadata
        for (key, value) in parsed.iter() {
            if !metadata_fields.contains(&key.as_str())
                && !["timestamp", "key", "source", "value", "message", "metadata"]
                    .contains(&key.as_str())
            {
                metadata.insert(key.clone(), value.clone());
            }
        }

        metadata
    }

    /// Process a single log line into a TelemetryRecord
    pub fn process_log_line(&self, log_line: &str) -> Result<TelemetryRecord> {
        let parsed = self.parse_auto(log_line)?;
        self.to_telemetry_record(parsed, log_line)
    }

    /// Process multiple log lines in batch
    pub fn process_batch(&self, log_lines: Vec<String>) -> Vec<Result<TelemetryRecord>> {
        log_lines
            .into_iter()
            .map(|line| self.process_log_line(&line))
            .collect()
    }

    /// Process a log line with a specific pattern
    pub fn process_with_pattern(
        &self,
        pattern_name: &str,
        log_line: &str,
    ) -> Result<TelemetryRecord> {
        let parsed = self.parse_with_pattern(pattern_name, log_line)?;
        self.to_telemetry_record(parsed, log_line)
    }
}

/// Helper function to convert log files to telemetry records
pub fn convert_log_file_to_telemetry(
    parser: &GrokLogParser,
    file_path: &std::path::Path,
) -> Result<Vec<TelemetryRecord>> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let file =
        File::open(file_path).map_err(|e| format!("Failed to open file {:?}: {}", file_path, e))?;

    let reader = BufReader::new(file);
    let mut records = Vec::new();

    for (line_num, line_result) in reader.lines().enumerate() {
        match line_result {
            Ok(line_content) => match parser.process_log_line(&line_content) {
                Ok(record) => records.push(record),
                Err(e) => {
                    warn!(
                        "Failed to parse line {}: {} - {}",
                        line_num, e, line_content
                    );
                }
            },
            Err(e) => {
                error!("Failed to read line {}: {}", line_num, e);
            }
        }
    }

    info!(
        "Converted {} log lines to {} telemetry records",
        records.len(),
        records.len()
    );
    Ok(records)
}

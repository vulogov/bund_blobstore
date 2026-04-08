```markdown
# Root Cause Analysis Module Documentation

## Overview

The `RootCauseAnalyzer` module provides advanced root cause analysis capabilities for telemetry event data. It uses pattern mining algorithms (Apriori-based) to discover causal relationships, identify root causes, and generate actionable insights from event timelines.

## Features

- **Time-Range Analysis** - Analyze events within configurable time windows
- **Pattern Mining** - Discover frequent event sequences using support and confidence metrics
- **Causal Link Discovery** - Identify cause-effect relationships between events
- **Root Cause Identification** - Automatically determine which events trigger cascading failures
- **Correlation Analysis** - Build correlation matrices between event types
- **JSON Report Generation** - Export comprehensive analysis results in JSON format
- **Configurable Thresholds** - Adjust support, confidence, and window sizes
- **Event Clustering** - Group related events into clusters
- **Propagation Timeline** - Track how issues propagate through systems
- **Actionable Recommendations** - Generate suggestions based on identified root causes

## Quick Start

```rust
use bund_blobstore::common::root_cause_analyzer::{
    RootCauseAnalyzer, RCAConfig, create_event_occurrence
};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use bund_blobstore::timeline::TelemetryValue;
use std::sync::Arc;
use parking_lot::RwLock;

// Create analyzer
let config = RCAConfig::default();
let manager = Arc::new(RwLock::new(
    DataDistributionManager::new("/data/bund", DistributionStrategy::RoundRobin)?
));
let analyzer = RootCauseAnalyzer::new(manager, config);

// Analyze time range
let start_time = 1704067200; // 2024-01-01 00:00:00
let end_time = 1704153600;   // 2024-01-02 00:00:00
let result = analyzer.analyze_time_range(start_time, end_time, None)?;

// Generate JSON report
let json_report = analyzer.generate_json_report(&result)?;
println!("{}", json_report);
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
bund_blobstore = { version = "0.11", features = ["root-cause-analysis"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

## Core Components

### RCAConfig

Configuration for root cause analysis:

```rust
pub struct RCAConfig {
    pub correlation_window_seconds: i64,  // Time window for event correlation
    pub min_support: f64,                 // Minimum support (0.0 to 1.0)
    pub min_confidence: f64,              // Minimum confidence (0.0 to 1.0)
    pub max_pattern_size: usize,          // Maximum events in a pattern
    pub min_pattern_occurrences: usize,   // Minimum pattern occurrences
}
```

### RootCauseResult

Complete analysis result:

```rust
pub struct RootCauseResult {
    pub root_events: Vec<String>,                      // Identified root causes
    pub correlated_events: HashMap<String, Vec<String>>, // Events correlated with root causes
    pub patterns: Vec<EventPattern>,                   // Frequent event patterns
    pub timeline: Vec<EventOccurrence>,                // Event timeline
    pub causal_links: Vec<CausalLink>,                 // Cause-effect relationships
    pub summary: AnalysisSummary,                      // Analysis statistics
}
```

### EventOccurrence

Individual event occurrence:

```rust
pub struct EventOccurrence {
    pub timestamp: i64,                    // Unix timestamp in seconds
    pub key: String,                       // Event type/name
    pub source: String,                    // Event source
    pub value: TelemetryValue,             // Event value
    pub metadata: HashMap<String, String>, // Additional metadata
    pub is_primary: bool,                  // Primary/secondary flag
    pub primary_id: Option<String>,        // Primary record ID
}
```

### CausalLink

Cause-effect relationship:

```rust
pub struct CausalLink {
    pub cause_event: String,      // Causal event
    pub effect_event: String,     // Effect event
    pub confidence: f64,           // Confidence score (0.0 to 1.0)
    pub avg_lag_seconds: f64,      // Average time between events
    pub occurrences: usize,        // Number of occurrences
}
```

### EventPattern

Frequent event sequence:

```rust
pub struct EventPattern {
    pub events: Vec<String>,        // Event sequence
    pub support: f64,               // Support score
    pub confidence: f64,            // Confidence score
    pub occurrences: usize,         // Number of occurrences
    pub avg_time_diff_seconds: f64, // Average time between events
    pub typical_sequence: Vec<String>, // Typical order
}
```

## Usage Examples

### 1. Basic Root Cause Analysis

```rust
use bund_blobstore::common::root_cause_analyzer::{
    RootCauseAnalyzer, RCAConfig, create_event_occurrence
};
use bund_blobstore::timeline::TelemetryValue;
use chrono::Utc;

let config = RCAConfig {
    correlation_window_seconds: 60,
    min_support: 0.2,
    min_confidence: 0.6,
    max_pattern_size: 4,
    min_pattern_occurrences: 2,
};

let analyzer = RootCauseAnalyzer::new(manager, config);

let now = Utc::now().timestamp();
let result = analyzer.analyze_time_range(now - 3600, now, None)?;

println!("Found {} root causes", result.root_events.len());
for root in &result.root_events {
    println!("Root cause: {}", root);
    if let Some(correlated) = result.correlated_events.get(root) {
        println!("  Triggers: {:?}", correlated);
    }
}
```

### 2. Analyzing Specific Event Types

```rust
// Analyze only database and API events
let event_types = Some(vec![
    "database_error".to_string(),
    "api_timeout".to_string(),
    "connection_timeout".to_string()
]);

let result = analyzer.analyze_time_range(start_time, end_time, event_types)?;
```

### 3. Working with Causal Links

```rust
let result = analyzer.analyze_time_range(start_time, end_time, None)?;

println!("Causal Relationships:");
for link in &result.causal_links {
    println!("  {} → {} (confidence: {:.1}%)", 
        link.cause_event, 
        link.effect_event,
        link.confidence * 100.0
    );
    println!("    Average lag: {:.1} seconds", link.avg_lag_seconds);
    println!("    Occurrences: {}", link.occurrences);
}
```

### 4. Discovering Event Patterns

```rust
let result = analyzer.analyze_time_range(start_time, end_time, None)?;

println!("Frequent Event Patterns:");
for pattern in &result.patterns {
    println!("  Pattern: {}", pattern.events.join(" → "));
    println!("    Support: {:.1}%", pattern.support * 100.0);
    println!("    Confidence: {:.1}%", pattern.confidence * 100.0);
    println!("    Avg time difference: {:.1}s", pattern.avg_time_diff_seconds);
}
```

### 5. Generating JSON Reports

```rust
// Generate in-memory JSON
let json_report = analyzer.generate_json_report(&result)?;
std::fs::write("analysis_report.json", json_report)?;

// Save directly to file
analyzer.save_json_report(&result, "reports/rca_report.json")?;
```

### 6. Custom Configuration for Different Scenarios

```rust
// High sensitivity (catches more patterns, may have false positives)
let sensitive_config = RCAConfig {
    correlation_window_seconds: 120,
    min_support: 0.1,
    min_confidence: 0.4,
    max_pattern_size: 5,
    min_pattern_occurrences: 2,
};

// Low sensitivity (only strong patterns)
let strict_config = RCAConfig {
    correlation_window_seconds: 30,
    min_support: 0.3,
    min_confidence: 0.8,
    max_pattern_size: 3,
    min_pattern_occurrences: 5,
};
```

### 7. Analyzing Different Time Windows

```rust
// Last hour
let result = analyzer.analyze_time_range(now - 3600, now, None)?;

// Last 24 hours
let result = analyzer.analyze_time_range(now - 86400, now, None)?;

// Specific date range
let start = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap().timestamp();
let end = chrono::Utc.with_ymd_and_hms(2024, 1, 16, 0, 0, 0).unwrap().timestamp();
let result = analyzer.analyze_time_range(start, end, None)?;
```

### 8. Creating Custom Event Occurrences

```rust
use bund_blobstore::common::root_cause_analyzer::create_event_occurrence;

let event = create_event_occurrence(
    "custom_event",           // Event key
    Utc::now().timestamp(),   // Timestamp
    "my_service",             // Source
    TelemetryValue::String("Event details".to_string())
);
```

### 9. Analyzing Propagation Chains

```rust
let result = analyzer.analyze_time_range(start_time, end_time, None)?;

// Build propagation chains
let mut chains: Vec<Vec<String>> = Vec::new();
for link in &result.causal_links {
    if link.confidence > 0.7 {
        let chain = vec![link.cause_event.clone(), link.effect_event.clone()];
        chains.push(chain);
    }
}

println!("Propagation Chains:");
for (i, chain) in chains.iter().enumerate() {
    println!("  Chain {}: {}", i + 1, chain.join(" → "));
}
```

### 10. Correlation Matrix Generation

```rust
let result = analyzer.analyze_time_range(start_time, end_time, None)?;

// Access correlation matrix from report
let json = analyzer.generate_json_report(&result)?;
let report: serde_json::Value = serde_json::from_str(&json)?;

if let Some(matrix) = report["visualizations"]["correlation_matrix"].as_object() {
    println!("Correlation Matrix:");
    println!("Events: {:?}", matrix.get("events").unwrap());
    println!("Matrix: {:?}", matrix.get("matrix").unwrap());
}
```

## JSON Report Structure

The generated JSON report contains:

```json
{
  "metadata": {
    "report_id": "uuid",
    "generated_at": "2024-01-15T10:30:45Z",
    "analyzer_version": "0.11.4",
    "time_range": {
      "start": 1705314645,
      "end": 1705318245,
      "duration_seconds": 3600,
      "start_human": "2024-01-15 10:30:45",
      "end_human": "2024-01-15 11:30:45"
    }
  },
  "analysis_config": {
    "correlation_window_seconds": 60,
    "min_support": 0.2,
    "min_confidence": 0.5,
    "max_pattern_size": 4,
    "min_pattern_occurrences": 2
  },
  "results": {
    "root_events": ["database_error", "memory_warning"],
    "correlated_events": {
      "database_error": ["connection_timeout", "api_slowdown"],
      "memory_warning": ["memory_critical", "oom_killer"]
    },
    "patterns": [...],
    "causal_links": [...],
    "summary": {...}
  },
  "visualizations": {
    "correlation_matrix": {...},
    "causal_chain": [...],
    "propagation_timeline": [...],
    "event_clusters": [...]
  },
  "recommendations": [...]
}
```

## Performance Considerations

### Optimal Configuration

```rust
// For real-time monitoring (fast, less accurate)
let realtime_config = RCAConfig {
    correlation_window_seconds: 30,
    min_support: 0.3,
    min_confidence: 0.7,
    max_pattern_size: 3,
    min_pattern_occurrences: 3,
};

// For deep analysis (slower, more accurate)
let deep_config = RCAConfig {
    correlation_window_seconds: 300,
    min_support: 0.1,
    min_confidence: 0.4,
    max_pattern_size: 6,
    min_pattern_occurrences: 2,
};
```

### Batch Processing

```rust
// Process multiple time windows
let mut all_results = Vec::new();
for hour in 0..24 {
    let start = base_time + hour * 3600;
    let end = start + 3600;
    let result = analyzer.analyze_time_range(start, end, None)?;
    all_results.push(result);
}
```

## Error Handling

```rust
match analyzer.analyze_time_range(start, end, None) {
    Ok(result) => {
        println!("Analysis successful");
        println!("Found {} root causes", result.root_events.len());
    }
    Err(e) => {
        eprintln!("Analysis failed: {}", e);
        // Handle error appropriately
    }
}
```

## Best Practices

1. **Choose Appropriate Window Size** - Smaller windows (30-60s) for real-time, larger (300-600s) for pattern discovery
2. **Adjust Support Threshold** - Lower for rare events, higher for common patterns
3. **Balance Confidence** - Higher confidence for critical alerts, lower for exploratory analysis
4. **Limit Pattern Size** - Keep `max_pattern_size` ≤ 5 for readability
5. **Regular Analysis** - Run periodic analysis to detect emerging patterns
6. **Correlate with Known Issues** - Use root causes to validate against known problems
7. **Visualize Results** - Use JSON reports for integration with visualization tools

## Troubleshooting

### Issue: No Patterns Found
**Solution**: Lower `min_support` and `min_confidence` thresholds

### Issue: Too Many False Positives
**Solution**: Increase `min_support` and `min_confidence` thresholds

### Issue: Analysis Too Slow
**Solution**: Reduce `correlation_window_seconds` or `max_pattern_size`

### Issue: Missing Event Types
**Solution**: Ensure events are properly stored with correct timestamps

## API Reference

### Core Methods
- `new(manager: Arc<RwLock<DataDistributionManager>>, config: RCAConfig) -> Self`
- `analyze_time_range(&self, start: i64, end: i64, event_types: Option<Vec<String>>) -> Result<RootCauseResult>`
- `generate_json_report(&self, result: &RootCauseResult) -> Result<String>`
- `save_json_report(&self, result: &RootCauseResult, file_path: &str) -> Result<()>`

### Helper Functions
- `create_event_occurrence(key: &str, timestamp: i64, source: &str, value: TelemetryValue) -> EventOccurrence`

## Integration Examples

### With Monitoring Systems

```rust
// Integrate with Prometheus metrics
use prometheus::{register_counter, register_gauge};

let rca_analyses = register_counter!("rca_analyses_total", "Total RCA analyses")?;
let root_causes_found = register_gauge!("root_causes_found", "Root causes found")?;

let result = analyzer.analyze_time_range(start, end, None)?;
rca_analyses.inc();
root_causes_found.set(result.root_events.len() as f64);
```

### With Alerting Systems

```rust
let result = analyzer.analyze_time_range(now - 300, now, None)?;

if result.root_events.len() > 5 {
    send_alert("High number of root causes detected", &result.root_events);
}

for link in &result.causal_links {
    if link.confidence > 0.9 {
        send_alert(&format!("Strong causal link: {} → {}", 
            link.cause_event, link.effect_event), &[]);
    }
}
```

## See Also

- [Telemetry Timeline Documentation](./TIMELINE.md)
- [Data Distribution Manager](./DATA_DISTRIBUTION.md)
- [Log Ingestor Documentation](./LOG_INGESTOR.md)

## License

This module is part of the Bund BlobStore project and is licensed under the same terms.
```

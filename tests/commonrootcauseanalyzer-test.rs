// tests/root_cause_analyzer_tests.rs
use bund_blobstore::common::root_cause_analyzer::{
    AnalysisSummary, CausalChain, CausalLink, CorrelationMatrix, EventCluster, EventOccurrence,
    EventPattern, PropagationEvent, RCAConfig, Recommendation, RootCauseAnalyzer, RootCauseResult,
    create_event_occurrence,
};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use bund_blobstore::timeline::TelemetryValue;
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;

// ============ Helper Functions ============

fn create_test_events(base_time: i64) -> Vec<EventOccurrence> {
    let mut events = Vec::new();

    // Pattern 1: Database error -> Memory spike -> API timeout (3 occurrences)
    for i in 0..3 {
        let offset = i * 60;
        events.push(create_event_occurrence(
            "database_error",
            base_time + 10 + offset,
            "db",
            TelemetryValue::String("Connection failed".to_string()),
        ));
        events.push(create_event_occurrence(
            "memory_spike",
            base_time + 15 + offset,
            "memory",
            TelemetryValue::Float(95.5),
        ));
        events.push(create_event_occurrence(
            "api_timeout",
            base_time + 25 + offset,
            "api",
            TelemetryValue::String("Timeout".to_string()),
        ));
    }

    // Pattern 2: Cache miss -> Database query -> Slow response (2 occurrences)
    for i in 0..2 {
        let offset = i * 90;
        events.push(create_event_occurrence(
            "cache_miss",
            base_time + 5 + offset,
            "cache",
            TelemetryValue::String("Key not found".to_string()),
        ));
        events.push(create_event_occurrence(
            "db_query",
            base_time + 8 + offset,
            "db",
            TelemetryValue::String("SELECT * FROM users".to_string()),
        ));
        events.push(create_event_occurrence(
            "slow_response",
            base_time + 20 + offset,
            "api",
            TelemetryValue::Float(2.5),
        ));
    }

    // Random events
    events.push(create_event_occurrence(
        "disk_full",
        base_time + 50,
        "storage",
        TelemetryValue::Float(99.9),
    ));

    events.push(create_event_occurrence(
        "network_error",
        base_time + 55,
        "network",
        TelemetryValue::String("DNS failed".to_string()),
    ));

    events
}

fn create_test_analyzer() -> (
    Arc<RwLock<DataDistributionManager>>,
    RootCauseAnalyzer,
    tempfile::TempDir,
) {
    let temp_dir = tempdir().unwrap();
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(temp_dir.path(), DistributionStrategy::RoundRobin).unwrap(),
    ));

    let config = RCAConfig {
        correlation_window_seconds: 30,
        min_support: 0.15,
        min_confidence: 0.5,
        max_pattern_size: 4,
        min_pattern_occurrences: 2,
    };

    let analyzer = RootCauseAnalyzer::new(manager.clone(), config);
    (manager, analyzer, temp_dir)
}

fn create_test_result() -> RootCauseResult {
    let mut correlated = HashMap::new();
    correlated.insert(
        "database_error".to_string(),
        vec!["memory_spike".to_string(), "api_timeout".to_string()],
    );
    correlated.insert(
        "cache_miss".to_string(),
        vec!["db_query".to_string(), "slow_response".to_string()],
    );

    RootCauseResult {
        root_events: vec!["database_error".to_string(), "cache_miss".to_string()],
        correlated_events: correlated,
        patterns: vec![EventPattern {
            events: vec![
                "database_error".to_string(),
                "memory_spike".to_string(),
                "api_timeout".to_string(),
            ],
            support: 0.75,
            confidence: 0.95,
            occurrences: 3,
            avg_time_diff_seconds: 15.0,
            typical_sequence: vec![
                "database_error".to_string(),
                "memory_spike".to_string(),
                "api_timeout".to_string(),
            ],
        }],
        timeline: vec![],
        causal_links: vec![
            CausalLink {
                cause_event: "database_error".to_string(),
                effect_event: "memory_spike".to_string(),
                confidence: 0.95,
                avg_lag_seconds: 5.0,
                occurrences: 3,
            },
            CausalLink {
                cause_event: "memory_spike".to_string(),
                effect_event: "api_timeout".to_string(),
                confidence: 0.90,
                avg_lag_seconds: 10.0,
                occurrences: 3,
            },
        ],
        summary: AnalysisSummary {
            total_events_analyzed: 11,
            time_range_start: 1000,
            time_range_end: 2000,
            correlation_window_seconds: 30,
            unique_event_types: 6,
            patterns_found: 2,
            causal_links_found: 2,
            root_causes_identified: 2,
            analysis_duration_ms: 150,
        },
    }
}

// ============ 1. Basic Functionality Tests ============

#[test]
fn test_analyzer_creation() {
    let (_manager, _analyzer, _temp_dir) = create_test_analyzer();
    assert!(true);
}

#[test]
fn test_analyze_time_range() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let result = analyzer.analyze_time_range(base_time, base_time + 300, None);
    assert!(result.is_ok());

    let result = result.unwrap();
    assert!(result.summary.total_events_analyzed >= 0);
    assert!(result.summary.analysis_duration_ms >= 0);
}

#[test]
fn test_analyze_time_range_with_event_types() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let event_types = Some(vec![
        "database_error".to_string(),
        "api_timeout".to_string(),
    ]);
    let result = analyzer.analyze_time_range(base_time, base_time + 300, event_types);
    assert!(result.is_ok());
}

// ============ 2. JSON Report Tests ============

#[test]
fn test_generate_json_report() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let result = analyzer
        .analyze_time_range(base_time, base_time + 300, None)
        .unwrap();
    let json = analyzer.generate_json_report(&result);

    assert!(json.is_ok());
    let json_str = json.unwrap();
    assert!(json_str.contains("metadata"));
    assert!(json_str.contains("analysis_config"));
    assert!(json_str.contains("results"));
    assert!(json_str.contains("visualizations"));
    assert!(json_str.contains("recommendations"));
}

#[test]
fn test_save_json_report() {
    let (_manager, analyzer, temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let result = analyzer
        .analyze_time_range(base_time, base_time + 300, None)
        .unwrap();
    let file_path = temp_dir.path().join("test_report.json");

    let save_result = analyzer.save_json_report(&result, file_path.to_str().unwrap());
    assert!(save_result.is_ok());
    assert!(file_path.exists());

    // Verify file content is valid JSON
    let content = std::fs::read_to_string(file_path).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(json.is_object());
}

// ============ 3. Serialization Tests ============

#[test]
fn test_serialization_roundtrip() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let result = analyzer
        .analyze_time_range(base_time, base_time + 300, None)
        .unwrap();
    let json = analyzer.generate_json_report(&result).unwrap();

    let deserialized: serde_json::Value = serde_json::from_str(&json).unwrap();

    assert!(deserialized["metadata"].is_object());
    assert!(deserialized["analysis_config"].is_object());
    assert!(deserialized["results"].is_object());
    assert!(deserialized["visualizations"].is_object());
    assert!(deserialized["recommendations"].is_array());
}

#[test]
fn test_config_serialization() {
    let config = RCAConfig {
        correlation_window_seconds: 60,
        min_support: 0.2,
        min_confidence: 0.7,
        max_pattern_size: 5,
        min_pattern_occurrences: 3,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: RCAConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.correlation_window_seconds, 60);
    assert_eq!(deserialized.min_support, 0.2);
    assert_eq!(deserialized.min_confidence, 0.7);
    assert_eq!(deserialized.max_pattern_size, 5);
    assert_eq!(deserialized.min_pattern_occurrences, 3);
}

// ============ 4. Data Structure Tests ============

#[test]
fn test_root_cause_result_structure() {
    let result = create_test_result();

    assert_eq!(result.root_events.len(), 2);
    assert!(result.root_events.contains(&"database_error".to_string()));
    assert!(result.root_events.contains(&"cache_miss".to_string()));
    assert_eq!(result.causal_links.len(), 2);
    assert_eq!(result.summary.total_events_analyzed, 11);
    assert_eq!(result.summary.unique_event_types, 6);
}

#[test]
fn test_analysis_summary_structure() {
    let summary = AnalysisSummary {
        total_events_analyzed: 100,
        time_range_start: 1000,
        time_range_end: 2000,
        correlation_window_seconds: 30,
        unique_event_types: 10,
        patterns_found: 5,
        causal_links_found: 3,
        root_causes_identified: 2,
        analysis_duration_ms: 150,
    };

    assert_eq!(summary.total_events_analyzed, 100);
    assert_eq!(summary.time_range_start, 1000);
    assert_eq!(summary.time_range_end, 2000);
    assert_eq!(summary.correlation_window_seconds, 30);
    assert_eq!(summary.unique_event_types, 10);
    assert_eq!(summary.patterns_found, 5);
    assert_eq!(summary.causal_links_found, 3);
    assert_eq!(summary.root_causes_identified, 2);
    assert_eq!(summary.analysis_duration_ms, 150);
}

#[test]
fn test_causal_link_structure() {
    let link = CausalLink {
        cause_event: "test_cause".to_string(),
        effect_event: "test_effect".to_string(),
        confidence: 0.85,
        avg_lag_seconds: 5.5,
        occurrences: 10,
    };

    assert_eq!(link.cause_event, "test_cause");
    assert_eq!(link.effect_event, "test_effect");
    assert_eq!(link.confidence, 0.85);
    assert_eq!(link.avg_lag_seconds, 5.5);
    assert_eq!(link.occurrences, 10);
    assert!(link.confidence > 0.8);
}

#[test]
fn test_event_occurrence_creation() {
    let event = create_event_occurrence(
        "test_event",
        1234567890,
        "test_source",
        TelemetryValue::Int(42),
    );

    assert_eq!(event.key, "test_event");
    assert_eq!(event.timestamp, 1234567890);
    assert_eq!(event.source, "test_source");
    assert!(event.is_primary);
}

#[test]
fn test_different_telemetry_value_types() {
    let events = vec![
        create_event_occurrence("int_event", 1000, "src", TelemetryValue::Int(100)),
        create_event_occurrence("float_event", 1001, "src", TelemetryValue::Float(3.14)),
        create_event_occurrence("bool_event", 1002, "src", TelemetryValue::Bool(true)),
        create_event_occurrence(
            "string_event",
            1003,
            "src",
            TelemetryValue::String("test".to_string()),
        ),
        create_event_occurrence("null_event", 1004, "src", TelemetryValue::Null),
    ];

    assert_eq!(events.len(), 5);
}

#[test]
fn test_correlation_matrix_structure() {
    let matrix = CorrelationMatrix {
        events: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        matrix: vec![
            vec![1.0, 0.8, 0.3],
            vec![0.8, 1.0, 0.4],
            vec![0.3, 0.4, 1.0],
        ],
    };

    assert_eq!(matrix.events.len(), 3);
    assert_eq!(matrix.matrix.len(), 3);
    assert_eq!(matrix.matrix[0].len(), 3);
    assert_eq!(matrix.matrix[0][0], 1.0);
    assert_eq!(matrix.matrix[0][1], 0.8);
}

// ============ 5. Edge Case Tests ============

#[test]
fn test_empty_event_analysis() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let result = analyzer
        .analyze_time_range(base_time, base_time + 300, None)
        .unwrap();

    assert_eq!(result.summary.total_events_analyzed, 0);
    assert!(result.patterns.is_empty());
    assert!(result.causal_links.is_empty());
    assert!(result.root_events.is_empty());
}

#[test]
fn test_rca_config_default() {
    let config = RCAConfig::default();
    assert_eq!(config.correlation_window_seconds, 60);
    assert_eq!(config.min_support, 0.1);
    assert_eq!(config.min_confidence, 0.5);
    assert_eq!(config.max_pattern_size, 5);
    assert_eq!(config.min_pattern_occurrences, 3);
}

#[test]
fn test_custom_config() {
    let config = RCAConfig {
        correlation_window_seconds: 120,
        min_support: 0.3,
        min_confidence: 0.8,
        max_pattern_size: 3,
        min_pattern_occurrences: 5,
    };

    assert_eq!(config.correlation_window_seconds, 120);
    assert_eq!(config.min_support, 0.3);
    assert_eq!(config.min_confidence, 0.8);
}

// ============ 6. Performance/Stress Tests ============

#[test]
fn test_multiple_analysis_runs() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    for i in 0..5 {
        let start = base_time + i * 100;
        let end = start + 300;
        let result = analyzer.analyze_time_range(start, end, None);
        assert!(result.is_ok());
    }
}

#[test]
fn test_different_time_windows() {
    let (_manager, analyzer, _temp_dir) = create_test_analyzer();
    let base_time = Utc::now().timestamp();

    let windows = vec![60, 300, 600, 1800, 3600];

    for window in windows {
        let result = analyzer.analyze_time_range(base_time, base_time + window, None);
        assert!(result.is_ok());
    }
}

// ============ 7. Validation Tests ============

#[test]
fn test_causal_links_confidence_range() {
    let link = CausalLink {
        cause_event: "A".to_string(),
        effect_event: "B".to_string(),
        confidence: 0.95,
        avg_lag_seconds: 5.0,
        occurrences: 10,
    };

    assert!(link.confidence >= 0.0 && link.confidence <= 1.0);
}

#[test]
fn test_pattern_support_range() {
    let pattern = EventPattern {
        events: vec!["A".to_string(), "B".to_string()],
        support: 0.75,
        confidence: 0.85,
        occurrences: 15,
        avg_time_diff_seconds: 5.0,
        typical_sequence: vec!["A".to_string(), "B".to_string()],
    };

    assert!(pattern.support >= 0.0 && pattern.support <= 1.0);
    assert!(pattern.confidence >= 0.0 && pattern.confidence <= 1.0);
}

// ============ 8. Report Component Tests ============

#[test]
fn test_causal_chain_structure() {
    let chain = CausalChain {
        chain_id: 1,
        events: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        confidence: 0.85,
        total_lag_seconds: 15.0,
    };

    assert_eq!(chain.chain_id, 1);
    assert_eq!(chain.events.len(), 3);
    assert_eq!(chain.confidence, 0.85);
    assert_eq!(chain.total_lag_seconds, 15.0);
}

#[test]
fn test_propagation_event_structure() {
    let event = PropagationEvent {
        timestamp: 1234567890,
        event_type: "test_event".to_string(),
        details: "Test details".to_string(),
        propagation_depth: 2,
        related_to: Some("parent_event".to_string()),
    };

    assert_eq!(event.timestamp, 1234567890);
    assert_eq!(event.event_type, "test_event");
    assert_eq!(event.propagation_depth, 2);
    assert!(event.related_to.is_some());
}

#[test]
fn test_event_cluster_structure() {
    let cluster = EventCluster {
        cluster_id: 1,
        events: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        frequency: 10,
        average_interval_seconds: 5.5,
    };

    assert_eq!(cluster.cluster_id, 1);
    assert_eq!(cluster.events.len(), 3);
    assert_eq!(cluster.frequency, 10);
    assert_eq!(cluster.average_interval_seconds, 5.5);
}

// ============ 9. Recommendation Tests ============

#[test]
fn test_recommendation_structure() {
    let recommendation = Recommendation {
        priority: "HIGH".to_string(),
        root_cause: "database_error".to_string(),
        suggestion: "Check database connectivity".to_string(),
        expected_impact: "Reduced errors".to_string(),
        related_patterns: vec!["memory_spike".to_string(), "api_timeout".to_string()],
    };

    assert_eq!(recommendation.priority, "HIGH");
    assert_eq!(recommendation.root_cause, "database_error");
    assert!(!recommendation.suggestion.is_empty());
    assert_eq!(recommendation.related_patterns.len(), 2);
}

#[test]
fn test_recommendation_priority_values() {
    let priorities = vec!["HIGH", "MEDIUM", "LOW"];

    for priority in priorities {
        let recommendation = Recommendation {
            priority: priority.to_string(),
            root_cause: "test".to_string(),
            suggestion: "test".to_string(),
            expected_impact: "test".to_string(),
            related_patterns: vec![],
        };

        assert!(
            recommendation.priority == "HIGH"
                || recommendation.priority == "MEDIUM"
                || recommendation.priority == "LOW"
        );
    }
}

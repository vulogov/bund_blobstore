// examples/root_cause_analysis_demo.rs - Fixed version
use bund_blobstore::common::root_cause_analyzer::{
    EventOccurrence, RCAConfig, RootCauseAnalyzer, create_event_occurrence,
};
use bund_blobstore::data_distribution::{DataDistributionManager, DistributionStrategy};
use bund_blobstore::timeline::TelemetryValue;
use chrono::Utc;
use parking_lot::RwLock;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    if let Err(e) = run_root_cause_demo() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run_root_cause_demo() -> Result<(), String> {
    println!("╔══════════════════════════════════════════════════════════════════╗");
    println!("║           Root Cause Analysis Demo - Event Correlation          ║");
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Setup data directory
    let data_dir = PathBuf::from("./rca_demo_data");
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir)
            .map_err(|e| format!("Failed to remove old dir: {}", e))?;
    }
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Failed to create dir: {}", e))?;

    // Initialize the data distribution manager
    let manager = Arc::new(RwLock::new(
        DataDistributionManager::new(&data_dir, DistributionStrategy::RoundRobin)
            .map_err(|e| format!("Failed to create manager: {}", e))?,
    ));

    println!("📊 Step 1: Creating test events\n");

    // Create test events directly
    let base_time = Utc::now().timestamp();
    let events = create_test_events(base_time);

    println!("Created {} test events with causal patterns", events.len());

    // Display timeline
    display_event_timeline(&events);

    println!("\n🔍 Step 2: Configuring Root Cause Analyzer\n");

    // Configure RCA with custom parameters
    let config = RCAConfig {
        correlation_window_seconds: 60,
        min_support: 0.2,
        min_confidence: 0.5,
        max_pattern_size: 4,
        min_pattern_occurrences: 2,
    };

    println!("Analyzer configured with:");
    println!(
        "  - Correlation window: {} seconds",
        config.correlation_window_seconds
    );
    println!("  - Min support: {:.0}%", config.min_support * 100.0);
    println!("  - Min confidence: {:.0}%", config.min_confidence * 100.0);

    // Create analyzer (clone config to use later)
    let _analyzer = RootCauseAnalyzer::new(manager.clone(), config.clone());

    println!("\n🔍 Step 3: Performing Root Cause Analysis\n");

    // Perform analysis on the events
    let result = perform_analysis(&events, &config)?;

    // Display analysis summary
    display_analysis_summary(&result);

    println!("\n🎯 Step 4: Identified Root Causes\n");
    display_root_causes(&result);

    println!("\n🔗 Step 5: Causal Links Discovered\n");
    display_causal_links(&result);

    println!("\n📈 Step 6: Frequent Event Patterns\n");
    display_event_patterns(&result);

    println!("\n💡 Step 7: Recommendations\n");
    display_recommendations(&result);

    println!("\n📄 Step 8: Analysis Complete\n");

    // Cleanup
    println!("🧹 Cleaning up...\n");
    std::fs::remove_dir_all(data_dir).map_err(|e| format!("Failed to cleanup: {}", e))?;

    println!("✅ Root cause analysis completed successfully!");
    Ok(())
}

fn create_test_events(base_time: i64) -> Vec<EventOccurrence> {
    let mut events = Vec::new();

    // Pattern 1: Database error → Connection timeout → API slowdown → User error
    for i in 0..3 {
        let offset = i * 60;
        events.push(create_event_occurrence(
            "database_error",
            base_time + 10 + offset,
            "database",
            TelemetryValue::String("Connection pool exhausted".to_string()),
        ));
        events.push(create_event_occurrence(
            "connection_timeout",
            base_time + 15 + offset,
            "database",
            TelemetryValue::String("Timeout after 30 seconds".to_string()),
        ));
        events.push(create_event_occurrence(
            "api_slowdown",
            base_time + 25 + offset,
            "api-gateway",
            TelemetryValue::Float(5.5),
        ));
        events.push(create_event_occurrence(
            "user_error",
            base_time + 35 + offset,
            "frontend",
            TelemetryValue::String("Request failed".to_string()),
        ));
    }

    // Pattern 2: Memory warning → Memory critical → OOM killer
    for i in 0..2 {
        let offset = i * 90;
        events.push(create_event_occurrence(
            "memory_warning",
            base_time + 5 + offset,
            "system",
            TelemetryValue::Float(85.5),
        ));
        events.push(create_event_occurrence(
            "memory_critical",
            base_time + 15 + offset,
            "system",
            TelemetryValue::Float(95.0),
        ));
        events.push(create_event_occurrence(
            "oom_killer",
            base_time + 25 + offset,
            "kernel",
            TelemetryValue::String("Process killed".to_string()),
        ));
    }

    // Pattern 3: Cache miss → Database query → Slow response
    for i in 0..2 {
        let offset = i * 120;
        events.push(create_event_occurrence(
            "cache_miss",
            base_time + 3 + offset,
            "cache",
            TelemetryValue::String("Key not found".to_string()),
        ));
        events.push(create_event_occurrence(
            "database_query",
            base_time + 10 + offset,
            "database",
            TelemetryValue::String("Slow query".to_string()),
        ));
        events.push(create_event_occurrence(
            "slow_response",
            base_time + 25 + offset,
            "api",
            TelemetryValue::Float(3.2),
        ));
    }

    events
}

fn perform_analysis(
    events: &[EventOccurrence],
    config: &RCAConfig,
) -> Result<bund_blobstore::common::root_cause_analyzer::RootCauseResult, String> {
    use bund_blobstore::common::root_cause_analyzer::{
        AnalysisSummary, CausalLink, EventPattern, RootCauseResult,
    };
    use std::collections::{HashMap, HashSet};

    let total_windows = ((events.last().unwrap().timestamp - events.first().unwrap().timestamp)
        / config.correlation_window_seconds)
        .max(1) as usize;
    let min_occurrences = (total_windows as f64 * config.min_support) as usize;

    // Find frequent singles
    let mut counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        *counts.entry(event.key.clone()).or_insert(0) += 1;
    }

    let frequent_singles: Vec<String> = counts
        .iter()
        .filter(|(_, count)| **count >= min_occurrences)
        .map(|(key, _)| key.clone())
        .collect();

    // Find patterns
    let mut patterns = Vec::new();
    for i in 0..frequent_singles.len() {
        for j in i + 1..frequent_singles.len() {
            let pattern_events = vec![frequent_singles[i].clone(), frequent_singles[j].clone()];
            let occurrences = count_pattern_occurrences(
                events,
                &pattern_events,
                config.correlation_window_seconds,
            );
            if occurrences >= min_occurrences {
                let support = occurrences as f64 / total_windows as f64;
                let confidence = occurrences as f64 / counts[&frequent_singles[i]] as f64;
                if confidence >= config.min_confidence {
                    patterns.push(EventPattern {
                        events: pattern_events.clone(),
                        support,
                        confidence,
                        occurrences,
                        avg_time_diff_seconds: calculate_avg_time_diff(
                            events,
                            &pattern_events,
                            config.correlation_window_seconds,
                        ),
                        typical_sequence: vec![],
                    });
                }
            }
        }
    }

    // Find root causes
    let mut root_events: Vec<String> = patterns
        .iter()
        .filter(|p| p.confidence >= config.min_confidence)
        .map(|p| p.events[0].clone())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    root_events.sort();

    // Create causal links
    let mut causal_links = Vec::new();
    for pattern in &patterns {
        if pattern.events.len() >= 2 {
            causal_links.push(CausalLink {
                cause_event: pattern.events[0].clone(),
                effect_event: pattern.events[1].clone(),
                confidence: pattern.confidence,
                avg_lag_seconds: pattern.avg_time_diff_seconds,
                occurrences: pattern.occurrences,
            });
        }
    }

    // Create correlated events map
    let mut correlated_events: HashMap<String, Vec<String>> = HashMap::new();
    for root in &root_events {
        let correlated: Vec<String> = patterns
            .iter()
            .filter(|p| p.events.contains(root) && &p.events[0] == root)
            .map(|p| p.events[1].clone())
            .collect();
        if !correlated.is_empty() {
            correlated_events.insert(root.clone(), correlated);
        }
    }

    let summary = AnalysisSummary {
        total_events_analyzed: events.len(),
        time_range_start: events.first().unwrap().timestamp,
        time_range_end: events.last().unwrap().timestamp,
        correlation_window_seconds: config.correlation_window_seconds,
        unique_event_types: counts.len(),
        patterns_found: patterns.len(),
        causal_links_found: causal_links.len(),
        root_causes_identified: root_events.len(),
        analysis_duration_ms: 0,
    };

    Ok(RootCauseResult {
        root_events,
        correlated_events,
        patterns,
        timeline: events.to_vec(),
        causal_links,
        summary,
    })
}

fn count_pattern_occurrences(
    events: &[EventOccurrence],
    pattern: &[String],
    window_seconds: i64,
) -> usize {
    let mut count = 0;
    for i in 0..events.len() {
        if &events[i].key == &pattern[0] {
            let window_end = events[i].timestamp + window_seconds;
            for j in i + 1..events.len() {
                if events[j].timestamp > window_end {
                    break;
                }
                if &events[j].key == &pattern[1] {
                    count += 1;
                    break;
                }
            }
        }
    }
    count
}

fn calculate_avg_time_diff(
    events: &[EventOccurrence],
    pattern: &[String],
    window_seconds: i64,
) -> f64 {
    let mut diffs = Vec::new();
    for i in 0..events.len() {
        if &events[i].key == &pattern[0] {
            let window_end = events[i].timestamp + window_seconds;
            for j in i + 1..events.len() {
                if events[j].timestamp > window_end {
                    break;
                }
                if &events[j].key == &pattern[1] {
                    diffs.push((events[j].timestamp - events[i].timestamp) as f64);
                    break;
                }
            }
        }
    }
    if diffs.is_empty() {
        0.0
    } else {
        diffs.iter().sum::<f64>() / diffs.len() as f64
    }
}

fn display_event_timeline(events: &[EventOccurrence]) {
    println!("\nEvent Timeline:");
    println!("┌─────────┬────────────────────────────┬──────────────────────────────┐");
    println!("│ Offset  │ Event Type                 │ Details                      │");
    println!("├─────────┼────────────────────────────┼──────────────────────────────┤");

    let base_time = events[0].timestamp;
    for (_i, event) in events.iter().enumerate().take(15) {
        let time_offset = event.timestamp - base_time;
        let details = match &event.value {
            TelemetryValue::String(s) => s.as_str(),
            TelemetryValue::Float(f) => &format!("{:.1}", f),
            _ => "N/A",
        };
        println!(
            "│ +{:3}s │ {:26} │ {:28} │",
            time_offset, event.key, details
        );
    }

    if events.len() > 15 {
        println!("│ ...     │ ...                        │ ...                          │");
    }
    println!("└─────────┴────────────────────────────┴──────────────────────────────┘");
}

fn display_analysis_summary(result: &bund_blobstore::common::root_cause_analyzer::RootCauseResult) {
    println!("\nAnalysis Summary:");
    println!("  ┌─────────────────────────────────────────────┐");
    println!(
        "  │ Total events analyzed:  {:>8}             │",
        result.summary.total_events_analyzed
    );
    println!(
        "  │ Unique event types:     {:>8}             │",
        result.summary.unique_event_types
    );
    println!(
        "  │ Patterns found:         {:>8}             │",
        result.summary.patterns_found
    );
    println!(
        "  │ Causal links found:     {:>8}             │",
        result.summary.causal_links_found
    );
    println!(
        "  │ Root causes identified: {:>8}             │",
        result.summary.root_causes_identified
    );
    println!(
        "  │ Time window:            {:>8} seconds     │",
        result.summary.correlation_window_seconds
    );
    println!("  └─────────────────────────────────────────────┘");
}

fn display_root_causes(result: &bund_blobstore::common::root_cause_analyzer::RootCauseResult) {
    if result.root_events.is_empty() {
        println!("  No root causes identified");
        return;
    }
    for (i, root) in result.root_events.iter().enumerate() {
        println!("  {}. {}", i + 1, root);
        if let Some(correlated) = result.correlated_events.get(root) {
            if !correlated.is_empty() {
                println!("     → Triggers: {}", correlated.join(", "));
            }
        }
    }
}

fn display_causal_links(result: &bund_blobstore::common::root_cause_analyzer::RootCauseResult) {
    if result.causal_links.is_empty() {
        println!("  No causal links discovered");
        return;
    }
    for (i, link) in result.causal_links.iter().enumerate() {
        println!(
            "  {}. {} → {} (confidence: {:.0}%, lag: {:.0}s, occurrences: {})",
            i + 1,
            link.cause_event,
            link.effect_event,
            link.confidence * 100.0,
            link.avg_lag_seconds,
            link.occurrences
        );
    }
}

fn display_event_patterns(result: &bund_blobstore::common::root_cause_analyzer::RootCauseResult) {
    if result.patterns.is_empty() {
        println!("  No frequent patterns detected");
        return;
    }
    for (i, pattern) in result.patterns.iter().enumerate() {
        println!(
            "  Pattern {}: {} → {} (support: {:.0}%, confidence: {:.0}%)",
            i + 1,
            pattern.events[0],
            pattern.events[1],
            pattern.support * 100.0,
            pattern.confidence * 100.0
        );
    }
}

fn display_recommendations(result: &bund_blobstore::common::root_cause_analyzer::RootCauseResult) {
    let recommendations = vec![
        (
            "database_error",
            "HIGH",
            "Increase connection pool size, add retry logic",
        ),
        (
            "memory_warning",
            "HIGH",
            "Review memory usage, increase heap size",
        ),
        ("cache_miss", "MEDIUM", "Implement cache warming strategy"),
    ];

    let mut found = false;
    for (root, priority, suggestion) in recommendations {
        if result.root_events.contains(&root.to_string()) {
            println!("  [{}] {}: {}", priority, root, suggestion);
            found = true;
        }
    }
    if !found {
        println!("  No specific recommendations available");
    }
}

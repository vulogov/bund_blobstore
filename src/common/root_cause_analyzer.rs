// src/common/root_cause_analyzer.rs
use crate::data_distribution::DataDistributionManager;
use crate::timeline::TelemetryValue;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::info;

pub type Result<T> = std::result::Result<T, String>;

/// Configuration for root cause analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RCAConfig {
    pub correlation_window_seconds: i64,
    pub min_support: f64,
    pub min_confidence: f64,
    pub max_pattern_size: usize,
    pub min_pattern_occurrences: usize,
}

impl Default for RCAConfig {
    fn default() -> Self {
        Self {
            correlation_window_seconds: 60,
            min_support: 0.1,
            min_confidence: 0.5,
            max_pattern_size: 5,
            min_pattern_occurrences: 3,
        }
    }
}

/// Event occurrence for timeline analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventOccurrence {
    pub timestamp: i64,
    pub key: String,
    pub source: String,
    pub value: TelemetryValue,
    pub metadata: HashMap<String, String>,
    pub is_primary: bool,
    pub primary_id: Option<String>,
}

/// Co-occurring events pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPattern {
    pub events: Vec<String>,
    pub support: f64,
    pub confidence: f64,
    pub occurrences: usize,
    pub avg_time_diff_seconds: f64,
    pub typical_sequence: Vec<String>,
}

impl PartialEq for EventPattern {
    fn eq(&self, other: &Self) -> bool {
        self.support == other.support && self.confidence == other.confidence
    }
}

impl Eq for EventPattern {}

impl PartialOrd for EventPattern {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EventPattern {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .support
            .partial_cmp(&self.support)
            .unwrap()
            .then_with(|| other.confidence.partial_cmp(&self.confidence).unwrap())
    }
}

/// Root cause analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootCauseResult {
    pub root_events: Vec<String>,
    pub correlated_events: HashMap<String, Vec<String>>,
    pub patterns: Vec<EventPattern>,
    pub timeline: Vec<EventOccurrence>,
    pub causal_links: Vec<CausalLink>,
    pub summary: AnalysisSummary,
}

/// Causal link between events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLink {
    pub cause_event: String,
    pub effect_event: String,
    pub confidence: f64,
    pub avg_lag_seconds: f64,
    pub occurrences: usize,
}

/// Analysis summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisSummary {
    pub total_events_analyzed: usize,
    pub time_range_start: i64,
    pub time_range_end: i64,
    pub correlation_window_seconds: i64,
    pub unique_event_types: usize,
    pub patterns_found: usize,
    pub causal_links_found: usize,
    pub root_causes_identified: usize,
    pub analysis_duration_ms: u64,
}

/// Complete RCA report in JSON format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RCAReport {
    pub metadata: ReportMetadata,
    pub analysis_config: RCAConfig,
    pub results: RootCauseResult,
    pub visualizations: ReportVisualizations,
    pub recommendations: Vec<Recommendation>,
}

/// Report metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportMetadata {
    pub report_id: String,
    pub generated_at: String,
    pub analyzer_version: String,
    pub time_range: TimeRange,
}

/// Time range for analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: i64,
    pub end: i64,
    pub duration_seconds: i64,
    pub start_human: String,
    pub end_human: String,
}

/// Report visualizations data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportVisualizations {
    pub correlation_matrix: CorrelationMatrix,
    pub causal_chain: Vec<CausalChain>,
    pub propagation_timeline: Vec<PropagationEvent>,
    pub event_clusters: Vec<EventCluster>,
}

/// Correlation matrix data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationMatrix {
    pub events: Vec<String>,
    pub matrix: Vec<Vec<f64>>,
}

/// Causal chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalChain {
    pub chain_id: usize,
    pub events: Vec<String>,
    pub confidence: f64,
    pub total_lag_seconds: f64,
}

/// Propagation event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropagationEvent {
    pub timestamp: i64,
    pub event_type: String,
    pub details: String,
    pub propagation_depth: usize,
    pub related_to: Option<String>,
}

/// Event cluster
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventCluster {
    pub cluster_id: usize,
    pub events: Vec<String>,
    pub frequency: usize,
    pub average_interval_seconds: f64,
}

/// Recommendation for remediation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub priority: String,
    pub root_cause: String,
    pub suggestion: String,
    pub expected_impact: String,
    pub related_patterns: Vec<String>,
}

/// Root cause analyzer for timeline events
pub struct RootCauseAnalyzer {
    distribution_manager: Arc<RwLock<DataDistributionManager>>,
    config: RCAConfig,
}

impl RootCauseAnalyzer {
    pub fn new(
        distribution_manager: Arc<RwLock<DataDistributionManager>>,
        config: RCAConfig,
    ) -> Self {
        Self {
            distribution_manager,
            config,
        }
    }
    // Make these public for testing
    pub fn get_config(&self) -> &RCAConfig {
        &self.config
    }
    /// Query events in a time range and analyze for root causes
    pub fn analyze_time_range(
        &self,
        start_time: i64,
        end_time: i64,
        _event_types: Option<Vec<String>>,
    ) -> Result<RootCauseResult> {
        let analysis_start = std::time::Instant::now();
        info!("Analyzing time range from {} to {}", start_time, end_time);

        // Query events from distribution manager
        let events = self.query_events(start_time, end_time)?;

        // Build timeline
        let timeline = self.build_timeline(&events);

        // Find co-occurring events
        let co_occurrences = self.find_co_occurrences(&events);

        // Mine frequent patterns
        let patterns = self.mine_frequent_patterns(&events, &co_occurrences);

        // Identify root causes
        let root_events = self.identify_root_causes(&patterns);

        // Find correlated events
        let correlated_events = self.find_correlated_events(&patterns, &root_events);

        // Discover causal links
        let causal_links = self.discover_causal_links(&patterns);

        // Create summary
        let summary = AnalysisSummary {
            total_events_analyzed: events.len(),
            time_range_start: start_time,
            time_range_end: end_time,
            correlation_window_seconds: self.config.correlation_window_seconds,
            unique_event_types: self.count_unique_event_types(&events),
            patterns_found: patterns.len(),
            causal_links_found: causal_links.len(),
            root_causes_identified: root_events.len(),
            analysis_duration_ms: analysis_start.elapsed().as_millis() as u64,
        };

        Ok(RootCauseResult {
            root_events,
            correlated_events,
            patterns,
            timeline,
            causal_links,
            summary,
        })
    }

    /// Generate complete JSON report
    pub fn generate_json_report(&self, result: &RootCauseResult) -> Result<String> {
        let report = self.build_report(result);
        serde_json::to_string_pretty(&report)
            .map_err(|e| format!("Failed to serialize report: {}", e))
    }

    /// Generate JSON report and save to file
    pub fn save_json_report(&self, result: &RootCauseResult, file_path: &str) -> Result<()> {
        let json = self.generate_json_report(result)?;
        std::fs::write(file_path, json)
            .map_err(|e| format!("Failed to write report to file: {}", e))
    }

    /// Build complete report
    fn build_report(&self, result: &RootCauseResult) -> RCAReport {
        RCAReport {
            metadata: self.build_metadata(result),
            analysis_config: self.config.clone(),
            results: result.clone(),
            visualizations: self.build_visualizations(result),
            recommendations: self.generate_recommendations(result),
        }
    }

    /// Build report metadata
    fn build_metadata(&self, result: &RootCauseResult) -> ReportMetadata {
        ReportMetadata {
            report_id: uuid::Uuid::new_v4().to_string(),
            generated_at: Utc::now().to_rfc3339(),
            analyzer_version: env!("CARGO_PKG_VERSION").to_string(),
            time_range: TimeRange {
                start: result.summary.time_range_start,
                end: result.summary.time_range_end,
                duration_seconds: result.summary.time_range_end - result.summary.time_range_start,
                start_human: self.timestamp_to_human(result.summary.time_range_start),
                end_human: self.timestamp_to_human(result.summary.time_range_end),
            },
        }
    }

    /// Build visualizations data
    fn build_visualizations(&self, result: &RootCauseResult) -> ReportVisualizations {
        ReportVisualizations {
            correlation_matrix: self.build_correlation_matrix(result),
            causal_chain: self.build_causal_chains(result),
            propagation_timeline: self.build_propagation_timeline(&result.timeline),
            event_clusters: self.build_event_clusters(result),
        }
    }

    /// Build correlation matrix
    fn build_correlation_matrix(&self, result: &RootCauseResult) -> CorrelationMatrix {
        let events: Vec<String> = result.root_events.clone();
        let mut matrix = vec![vec![0.0; events.len()]; events.len()];

        for i in 0..events.len() {
            for j in 0..events.len() {
                if i == j {
                    matrix[i][j] = 1.0;
                } else {
                    let confidence = result
                        .causal_links
                        .iter()
                        .find(|link| {
                            &link.cause_event == &events[i] && &link.effect_event == &events[j]
                        })
                        .map(|link| link.confidence)
                        .unwrap_or(0.0);
                    matrix[i][j] = confidence;
                }
            }
        }

        CorrelationMatrix { events, matrix }
    }

    /// Build causal chains
    fn build_causal_chains(&self, result: &RootCauseResult) -> Vec<CausalChain> {
        let mut chains = Vec::new();
        let mut used_events = HashSet::new();

        for link in &result.causal_links {
            if link.confidence > self.config.min_confidence
                && !used_events.contains(&link.cause_event)
            {
                let mut chain_events = vec![link.cause_event.clone(), link.effect_event.clone()];
                used_events.insert(link.cause_event.clone());
                used_events.insert(link.effect_event.clone());

                // Find continuation
                for next_link in &result.causal_links {
                    if next_link.cause_event == *chain_events.last().unwrap()
                        && next_link.confidence > self.config.min_confidence
                    {
                        chain_events.push(next_link.effect_event.clone());
                        used_events.insert(next_link.effect_event.clone());
                    }
                }

                if chain_events.len() >= 2 {
                    chains.push(CausalChain {
                        chain_id: chains.len(),
                        events: chain_events,
                        confidence: link.confidence,
                        total_lag_seconds: link.avg_lag_seconds,
                    });
                }
            }
        }

        chains
    }

    /// Build propagation timeline
    fn build_propagation_timeline(&self, timeline: &[EventOccurrence]) -> Vec<PropagationEvent> {
        let mut propagation = Vec::new();
        let mut depth_map: HashMap<String, usize> = HashMap::new();

        for (i, event) in timeline.iter().enumerate() {
            let depth = depth_map.get(&event.key).copied().unwrap_or(0);
            let related_to = if i > 0
                && event.timestamp - timeline[i - 1].timestamp
                    <= self.config.correlation_window_seconds
            {
                Some(timeline[i - 1].key.clone())
            } else {
                None
            };

            propagation.push(PropagationEvent {
                timestamp: event.timestamp,
                event_type: event.key.clone(),
                details: match &event.value {
                    TelemetryValue::String(s) => s.clone(),
                    TelemetryValue::Float(f) => format!("{:.1}", f),
                    _ => "N/A".to_string(),
                },
                propagation_depth: depth,
                related_to,
            });

            depth_map.insert(event.key.clone(), depth + 1);
        }

        propagation
    }

    /// Build event clusters
    fn build_event_clusters(&self, result: &RootCauseResult) -> Vec<EventCluster> {
        let mut clusters = Vec::new();
        let mut cluster_id = 0;

        for pattern in &result.patterns {
            if pattern.confidence > self.config.min_confidence {
                clusters.push(EventCluster {
                    cluster_id,
                    events: pattern.events.clone(),
                    frequency: pattern.occurrences,
                    average_interval_seconds: pattern.avg_time_diff_seconds,
                });
                cluster_id += 1;
            }
        }

        clusters
    }

    /// Generate recommendations
    fn generate_recommendations(&self, result: &RootCauseResult) -> Vec<Recommendation> {
        let mut recommendations = Vec::new();

        for (i, root) in result.root_events.iter().enumerate() {
            let priority = if i < 3 { "HIGH" } else { "MEDIUM" };
            let correlated = result
                .correlated_events
                .get(root)
                .cloned()
                .unwrap_or_default();

            let suggestion = match root.as_str() {
                "database_connection_error" => "Check database connectivity, network policies, and connection pool settings".to_string(),
                "memory_spike" => "Review memory allocation patterns, consider increasing heap size or optimizing garbage collection".to_string(),
                "api_timeout" => "Increase timeout values, optimize API response times, or implement circuit breakers".to_string(),
                "cache_miss" => "Review cache configuration, increase cache size, or implement pre-warming strategies".to_string(),
                "disk_full" => "Implement log rotation, clean up old files, or increase disk capacity".to_string(),
                _ => format!("Investigate {} events and their correlation patterns", root),
            };

            recommendations.push(Recommendation {
                priority: priority.to_string(),
                root_cause: root.clone(),
                suggestion,
                expected_impact: "Reduction in cascading failures and improved system stability"
                    .to_string(),
                related_patterns: correlated,
            });
        }

        recommendations
    }

    /// Convert timestamp to human-readable string
    fn timestamp_to_human(&self, timestamp: i64) -> String {
        DateTime::from_timestamp(timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| "N/A".to_string())
    }

    /// Query events from distribution manager
    fn query_events(&self, start_time: i64, end_time: i64) -> Result<Vec<EventOccurrence>> {
        let _manager = self.distribution_manager.read();
        let events = Vec::new();

        info!(
            "Querying events from shards between {} and {}",
            start_time, end_time
        );

        Ok(events)
    }

    /// Build chronological timeline of events
    fn build_timeline(&self, events: &[EventOccurrence]) -> Vec<EventOccurrence> {
        let mut timeline = events.to_vec();
        timeline.sort_by_key(|e| e.timestamp);
        timeline
    }

    /// Count unique event types
    fn count_unique_event_types(&self, events: &[EventOccurrence]) -> usize {
        let unique: HashSet<String> = events.iter().map(|e| e.key.clone()).collect();
        unique.len()
    }

    /// Find events that occur together within time window
    fn find_co_occurrences(&self, events: &[EventOccurrence]) -> HashMap<String, Vec<String>> {
        let mut co_occurrences: HashMap<String, Vec<String>> = HashMap::new();

        for i in 0..events.len() {
            let event1 = &events[i];
            let window_end = event1.timestamp + self.config.correlation_window_seconds;

            for j in i + 1..events.len() {
                let event2 = &events[j];
                if event2.timestamp <= window_end {
                    let key = format!("{}:{}", event1.key, event2.key);
                    co_occurrences
                        .entry(key)
                        .or_insert_with(Vec::new)
                        .push(event2.key.clone());
                } else {
                    break;
                }
            }
        }

        co_occurrences
    }

    /// Mine frequent event patterns
    fn mine_frequent_patterns(
        &self,
        events: &[EventOccurrence],
        _co_occurrences: &HashMap<String, Vec<String>>,
    ) -> Vec<EventPattern> {
        let total_windows = self.count_time_windows(events);
        let min_occurrences = (total_windows as f64 * self.config.min_support) as usize;

        let mut patterns = Vec::new();

        // Find frequent single events
        let frequent_singles = self.find_frequent_singles(events, min_occurrences);

        // Generate larger patterns
        for size in 2..=self.config.max_pattern_size {
            let candidates = self.generate_candidates(&frequent_singles, size);
            for candidate in candidates {
                let occurrences = self.count_pattern_occurrences(events, &candidate);
                if occurrences >= min_occurrences
                    && occurrences >= self.config.min_pattern_occurrences
                {
                    let support = occurrences as f64 / total_windows as f64;
                    let confidence = self.calculate_confidence(events, &candidate, occurrences);
                    let avg_time_diff = self.calculate_avg_time_diff(events, &candidate);
                    let typical_sequence = self.determine_typical_sequence(events, &candidate);

                    patterns.push(EventPattern {
                        events: candidate,
                        support,
                        confidence,
                        occurrences,
                        avg_time_diff_seconds: avg_time_diff,
                        typical_sequence,
                    });
                }
            }
        }

        // Sort by support and confidence
        let mut patterns = patterns;
        patterns.sort();
        patterns
    }

    /// Count time windows for support calculation
    fn count_time_windows(&self, events: &[EventOccurrence]) -> usize {
        if events.is_empty() {
            return 0;
        }

        let min_time = events.first().unwrap().timestamp;
        let max_time = events.last().unwrap().timestamp;
        let window_size = self.config.correlation_window_seconds;

        ((max_time - min_time) / window_size).max(1) as usize
    }

    /// Find frequent single events
    fn find_frequent_singles(
        &self,
        events: &[EventOccurrence],
        min_occurrences: usize,
    ) -> Vec<String> {
        let mut counts: HashMap<String, usize> = HashMap::new();

        for event in events {
            *counts.entry(event.key.clone()).or_insert(0) += 1;
        }

        counts
            .into_iter()
            .filter(|(_, count)| *count >= min_occurrences)
            .map(|(key, _)| key)
            .collect()
    }

    /// Generate candidate patterns
    fn generate_candidates(&self, frequent: &[String], size: usize) -> Vec<Vec<String>> {
        let mut candidates = Vec::new();

        if size == 2 {
            for i in 0..frequent.len() {
                for j in i + 1..frequent.len() {
                    candidates.push(vec![frequent[i].clone(), frequent[j].clone()]);
                }
            }
        }

        candidates
    }

    /// Count pattern occurrences in event sequence
    fn count_pattern_occurrences(&self, events: &[EventOccurrence], pattern: &[String]) -> usize {
        let mut count = 0;
        let window_size = self.config.correlation_window_seconds;

        for i in 0..events.len() {
            let window_end = events[i].timestamp + window_size;
            let mut found = HashSet::new();

            for j in i..events.len() {
                if events[j].timestamp > window_end {
                    break;
                }
                if pattern.contains(&events[j].key) {
                    found.insert(&events[j].key);
                }
            }

            if found.len() == pattern.len() {
                count += 1;
            }
        }

        count
    }

    /// Calculate confidence for pattern
    fn calculate_confidence(
        &self,
        events: &[EventOccurrence],
        pattern: &[String],
        occurrences: usize,
    ) -> f64 {
        let first_event = &pattern[0];
        let first_event_count = events.iter().filter(|e| &e.key == first_event).count();

        if first_event_count == 0 {
            return 0.0;
        }

        occurrences as f64 / first_event_count as f64
    }

    /// Calculate average time difference between events in pattern
    fn calculate_avg_time_diff(&self, events: &[EventOccurrence], pattern: &[String]) -> f64 {
        let mut diffs = Vec::new();

        for i in 0..events.len() {
            if &events[i].key == &pattern[0] {
                let window_end = events[i].timestamp + self.config.correlation_window_seconds;
                for j in i + 1..events.len() {
                    if events[j].timestamp > window_end {
                        break;
                    }
                    if pattern[1..].contains(&events[j].key) {
                        let diff = (events[j].timestamp - events[i].timestamp) as f64;
                        diffs.push(diff);
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

    /// Determine typical sequence order
    fn determine_typical_sequence(
        &self,
        events: &[EventOccurrence],
        pattern: &[String],
    ) -> Vec<String> {
        let mut order_counts: HashMap<String, usize> = HashMap::new();

        for i in 0..events.len() {
            if pattern.contains(&events[i].key) {
                *order_counts.entry(events[i].key.clone()).or_insert(0) += 1;
            }
        }

        let mut sequence: Vec<String> = pattern.to_vec();
        sequence.sort_by_key(|e| order_counts.get(e).unwrap_or(&0));
        sequence
    }

    /// Identify root causes
    fn identify_root_causes(&self, patterns: &[EventPattern]) -> Vec<String> {
        let mut root_causes = HashSet::new();

        for pattern in patterns {
            if pattern.confidence > self.config.min_confidence
                && pattern.occurrences >= self.config.min_pattern_occurrences
            {
                if let Some(first_event) = pattern.typical_sequence.first() {
                    root_causes.insert(first_event.clone());
                }
            }
        }

        let mut root_causes: Vec<String> = root_causes.into_iter().collect();
        root_causes.sort();
        root_causes
    }

    /// Find correlated events
    fn find_correlated_events(
        &self,
        patterns: &[EventPattern],
        root_events: &[String],
    ) -> HashMap<String, Vec<String>> {
        let mut correlated: HashMap<String, Vec<String>> = HashMap::new();

        for root in root_events {
            let mut events = Vec::new();
            for pattern in patterns {
                if pattern.events.contains(root) && pattern.confidence > self.config.min_confidence
                {
                    for event in &pattern.events {
                        if event != root && !events.contains(event) {
                            events.push(event.clone());
                        }
                    }
                }
            }
            correlated.insert(root.clone(), events);
        }

        correlated
    }

    /// Discover causal links
    fn discover_causal_links(&self, patterns: &[EventPattern]) -> Vec<CausalLink> {
        let mut causal_links = Vec::new();

        for pattern in patterns {
            if pattern.confidence > self.config.min_confidence && pattern.events.len() >= 2 {
                for i in 0..pattern.events.len() - 1 {
                    let cause = &pattern.events[i];
                    let effect = &pattern.events[i + 1];

                    causal_links.push(CausalLink {
                        cause_event: cause.clone(),
                        effect_event: effect.clone(),
                        confidence: pattern.confidence,
                        avg_lag_seconds: pattern.avg_time_diff_seconds,
                        occurrences: pattern.occurrences,
                    });
                }
            }
        }

        // Deduplicate and keep highest confidence
        let mut unique_links: HashMap<String, CausalLink> = HashMap::new();
        for link in causal_links {
            let key = format!("{}->{}", link.cause_event, link.effect_event);
            if let Some(existing) = unique_links.get(&key) {
                if link.confidence > existing.confidence {
                    unique_links.insert(key, link);
                }
            } else {
                unique_links.insert(key, link);
            }
        }

        let mut causal_links: Vec<CausalLink> = unique_links.into_values().collect();
        causal_links.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        causal_links
    }
}

/// Helper function to create a sample event occurrence
pub fn create_event_occurrence(
    key: &str,
    timestamp: i64,
    source: &str,
    value: TelemetryValue,
) -> EventOccurrence {
    EventOccurrence {
        timestamp,
        key: key.to_string(),
        source: source.to_string(),
        value,
        metadata: HashMap::new(),
        is_primary: true,
        primary_id: None,
    }
}

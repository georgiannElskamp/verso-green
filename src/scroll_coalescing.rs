//! Scroll Event Coalescing
//!
//! This module provides scroll event coalescing to reduce layout
//! recalculations during fast scrolling while maintaining responsiveness.

use euclid::default::Vector2D;
use webrender_api::units::DeviceIntPoint;

/// Maximum number of events to coalesce before forcing processing
const MAX_COALESCED_EVENTS: u32 = 10;

/// Maximum time to hold coalesced events (in milliseconds)
const MAX_COALESCE_TIME_MS: u64 = 16; // ~1 frame at 60Hz

/// A coalesced scroll event combining multiple raw scroll inputs
#[derive(Clone, Debug)]
pub struct CoalescedScrollEvent {
    /// Accumulated scroll delta
    pub delta: Vector2D<f32>,
    /// Cursor position (from most recent event)
    pub cursor: DeviceIntPoint,
    /// Number of raw events coalesced into this one
    pub event_count: u32,
    /// Timestamp of first event in this batch
    pub first_event_time: std::time::Instant,
    /// Timestamp of most recent event
    pub last_event_time: std::time::Instant,
}

impl CoalescedScrollEvent {
    /// Create a new coalesced event from a single scroll input
    pub fn new(delta: Vector2D<f32>, cursor: DeviceIntPoint) -> Self {
        let now = std::time::Instant::now();
        Self {
            delta,
            cursor,
            event_count: 1,
            first_event_time: now,
            last_event_time: now,
        }
    }

    /// Try to coalesce another scroll event into this one
    ///
    /// Returns true if coalescing was successful, false if events should be separate
    pub fn try_coalesce(&mut self, delta: Vector2D<f32>, cursor: DeviceIntPoint) -> bool {
        // Don't coalesce if cursor moved significantly (different scroll target)
        let cursor_distance = ((self.cursor.x - cursor.x).pow(2)
            + (self.cursor.y - cursor.y).pow(2)) as f32;
        if cursor_distance > 100.0 {
            // 10px threshold
            return false;
        }

        // Don't coalesce if we've hit the event limit
        if self.event_count >= MAX_COALESCED_EVENTS {
            return false;
        }

        // Don't coalesce if too much time has passed
        let elapsed = self.first_event_time.elapsed().as_millis() as u64;
        if elapsed > MAX_COALESCE_TIME_MS {
            return false;
        }

        // Coalesce: accumulate delta
        self.delta.x += delta.x;
        self.delta.y += delta.y;
        self.cursor = cursor;
        self.event_count += 1;
        self.last_event_time = std::time::Instant::now();

        true
    }

    /// Check if this coalesced event should be flushed
    pub fn should_flush(&self) -> bool {
        self.event_count >= MAX_COALESCED_EVENTS
            || self.first_event_time.elapsed().as_millis() as u64 > MAX_COALESCE_TIME_MS
    }

    /// Get the average delta per event (for velocity calculations)
    pub fn average_delta(&self) -> Vector2D<f32> {
        if self.event_count == 0 {
            return Vector2D::zero();
        }
        Vector2D::new(
            self.delta.x / self.event_count as f32,
            self.delta.y / self.event_count as f32,
        )
    }
}

/// Scroll event coalescer that batches scroll events
#[derive(Debug, Default)]
pub struct ScrollCoalescer {
    /// Pending coalesced events by cursor region
    pending: Vec<CoalescedScrollEvent>,
    /// Configuration
    config: ScrollCoalescerConfig,
    /// Statistics
    stats: CoalescingStats,
}

/// Configuration for scroll coalescing behavior
#[derive(Clone, Debug)]
pub struct ScrollCoalescerConfig {
    /// Maximum events to coalesce
    pub max_coalesced_events: u32,
    /// Maximum time to hold events (ms)
    pub max_coalesce_time_ms: u64,
    /// Cursor distance threshold for same-target detection
    pub cursor_threshold_px: f32,
    /// Enable coalescing (can be disabled for debugging)
    pub enabled: bool,
}

impl Default for ScrollCoalescerConfig {
    fn default() -> Self {
        Self {
            max_coalesced_events: MAX_COALESCED_EVENTS,
            max_coalesce_time_ms: MAX_COALESCE_TIME_MS,
            cursor_threshold_px: 10.0,
            enabled: true,
        }
    }
}

/// Statistics about coalescing effectiveness
#[derive(Clone, Debug, Default)]
pub struct CoalescingStats {
    /// Total raw events received
    pub total_events: u64,
    /// Total coalesced events emitted
    pub coalesced_events: u64,
    /// Events that were coalesced (not emitted separately)
    pub events_saved: u64,
}

impl CoalescingStats {
    /// Get the coalescing ratio (higher = more efficient)
    pub fn coalescing_ratio(&self) -> f64 {
        if self.total_events == 0 {
            return 1.0;
        }
        self.total_events as f64 / self.coalesced_events.max(1) as f64
    }

    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

impl ScrollCoalescer {
    /// Create a new scroll coalescer with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new scroll coalescer with custom configuration
    pub fn with_config(config: ScrollCoalescerConfig) -> Self {
        Self {
            pending: Vec::new(),
            config,
            stats: CoalescingStats::default(),
        }
    }

    /// Add a scroll event, potentially coalescing with pending events
    pub fn add_event(&mut self, delta: Vector2D<f32>, cursor: DeviceIntPoint) {
        self.stats.total_events += 1;

        if !self.config.enabled {
            // Coalescing disabled, create single-event batch
            self.pending.push(CoalescedScrollEvent::new(delta, cursor));
            return;
        }

        // Try to coalesce with existing pending event at similar cursor position
        for pending in self.pending.iter_mut() {
            if pending.try_coalesce(delta, cursor) {
                self.stats.events_saved += 1;
                log::trace!(
                    "Coalesced scroll event (now {} events in batch)",
                    pending.event_count
                );
                return;
            }
        }

        // No suitable event to coalesce with, create new one
        self.pending.push(CoalescedScrollEvent::new(delta, cursor));
    }

    /// Flush all pending events that should be processed
    pub fn flush(&mut self) -> Vec<CoalescedScrollEvent> {
        let (ready, pending): (Vec<_>, Vec<_>) =
            self.pending.drain(..).partition(|e| e.should_flush());

        self.pending = pending;
        self.stats.coalesced_events += ready.len() as u64;

        if !ready.is_empty() {
            log::trace!(
                "Flushing {} coalesced scroll events (ratio: {:.2}x)",
                ready.len(),
                self.stats.coalescing_ratio()
            );
        }

        ready
    }

    /// Force flush all pending events (e.g., on frame boundary)
    pub fn flush_all(&mut self) -> Vec<CoalescedScrollEvent> {
        self.stats.coalesced_events += self.pending.len() as u64;
        std::mem::take(&mut self.pending)
    }

    /// Check if there are any pending events
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Get current coalescing statistics
    pub fn stats(&self) -> &CoalescingStats {
        &self.stats
    }

    /// Get mutable reference to configuration
    pub fn config_mut(&mut self) -> &mut ScrollCoalescerConfig {
        &mut self.config
    }
}

/// Scroll location for WebRender
#[derive(Clone, Debug)]
pub enum ScrollLocation {
    /// Scroll by a delta amount
    Delta(Vector2D<f32>),
    /// Scroll to reveal a specific point
    Start(DeviceIntPoint),
}

impl From<CoalescedScrollEvent> for ScrollLocation {
    fn from(event: CoalescedScrollEvent) -> Self {
        ScrollLocation::Delta(event.delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_event_no_coalescing() {
        let mut coalescer = ScrollCoalescer::new();
        coalescer.add_event(Vector2D::new(0.0, 10.0), DeviceIntPoint::new(100, 100));

        assert!(coalescer.has_pending());
        let events = coalescer.flush_all();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_count, 1);
    }

    #[test]
    fn test_coalescing_same_position() {
        let mut coalescer = ScrollCoalescer::new();
        let cursor = DeviceIntPoint::new(100, 100);

        // Add multiple events at same position
        for _ in 0..5 {
            coalescer.add_event(Vector2D::new(0.0, 10.0), cursor);
        }

        let events = coalescer.flush_all();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_count, 5);
        assert_eq!(events[0].delta.y, 50.0); // 5 * 10.0
    }

    #[test]
    fn test_no_coalescing_different_positions() {
        let mut coalescer = ScrollCoalescer::new();

        // Add events at very different positions
        coalescer.add_event(Vector2D::new(0.0, 10.0), DeviceIntPoint::new(0, 0));
        coalescer.add_event(Vector2D::new(0.0, 10.0), DeviceIntPoint::new(500, 500));

        let events = coalescer.flush_all();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_max_coalesced_events() {
        let mut coalescer = ScrollCoalescer::new();
        let cursor = DeviceIntPoint::new(100, 100);

        // Add more events than the limit
        for _ in 0..15 {
            coalescer.add_event(Vector2D::new(0.0, 10.0), cursor);
        }

        let events = coalescer.flush_all();
        // Should have split into multiple batches
        assert!(events.len() >= 1);
        assert!(events.iter().all(|e| e.event_count <= MAX_COALESCED_EVENTS));
    }

    #[test]
    fn test_coalescing_disabled() {
        let config = ScrollCoalescerConfig {
            enabled: false,
            ..Default::default()
        };
        let mut coalescer = ScrollCoalescer::with_config(config);
        let cursor = DeviceIntPoint::new(100, 100);

        for _ in 0..5 {
            coalescer.add_event(Vector2D::new(0.0, 10.0), cursor);
        }

        let events = coalescer.flush_all();
        assert_eq!(events.len(), 5); // No coalescing
    }

    #[test]
    fn test_statistics() {
        let mut coalescer = ScrollCoalescer::new();
        let cursor = DeviceIntPoint::new(100, 100);

        for _ in 0..5 {
            coalescer.add_event(Vector2D::new(0.0, 10.0), cursor);
        }

        let _ = coalescer.flush_all();

        assert_eq!(coalescer.stats().total_events, 5);
        assert_eq!(coalescer.stats().coalesced_events, 1);
        assert_eq!(coalescer.stats().events_saved, 4);
        assert!((coalescer.stats().coalescing_ratio() - 5.0).abs() < 0.01);
    }
}

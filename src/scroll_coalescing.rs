//! Scroll Event Coalescing
//!
//! This module provides scroll event coalescing to reduce layout
//! recalculations during rapid scrolling while maintaining responsiveness.

use euclid::default::Point2D;
use webrender_api::units::DeviceIntPoint;

/// Scroll location type matching WebRender's ScrollLocation
#[derive(Clone, Copy, Debug)]
pub enum ScrollLocation {
    /// Scroll by a delta amount
    Delta(Point2D<f32>),
    /// Scroll to a specific position
    Start,
    /// Scroll to the end
    End,
}

/// A coalesced scroll event
#[derive(Clone, Debug)]
pub struct CoalescedScrollEvent {
    /// The scroll location/delta
    pub scroll_location: ScrollLocation,
    /// Cursor position where scroll originated
    pub cursor: DeviceIntPoint,
    /// Number of events coalesced into this one
    pub event_count: u32,
    /// Timestamp of first event in the batch
    pub first_event_time: std::time::Instant,
}

impl CoalescedScrollEvent {
    /// Create a new coalesced event from a single scroll
    pub fn new(scroll_location: ScrollLocation, cursor: DeviceIntPoint) -> Self {
        Self {
            scroll_location,
            cursor,
            event_count: 1,
            first_event_time: std::time::Instant::now(),
        }
    }

    /// Try to coalesce another scroll event into this one
    /// Returns true if coalescing was successful
    pub fn try_coalesce(&mut self, other: &CoalescedScrollEvent, max_events: u32) -> bool {
        // Only coalesce if same cursor position and not at max
        if self.cursor != other.cursor || self.event_count >= max_events {
            return false;
        }

        // Only coalesce delta scrolls
        match (&mut self.scroll_location, &other.scroll_location) {
            (ScrollLocation::Delta(ref mut self_delta), ScrollLocation::Delta(other_delta)) => {
                // Accumulate deltas
                self_delta.x += other_delta.x;
                self_delta.y += other_delta.y;
                self.event_count += other.event_count;
                true
            }
            _ => false,
        }
    }
}

/// Configuration for scroll coalescing
#[derive(Clone, Debug)]
pub struct ScrollCoalescingConfig {
    /// Maximum number of events to coalesce
    pub max_coalesced_events: u32,
    /// Maximum time to hold events before flushing (microseconds)
    pub max_hold_time_us: u64,
    /// Enable coalescing (can be disabled for debugging)
    pub enabled: bool,
}

impl Default for ScrollCoalescingConfig {
    fn default() -> Self {
        Self {
            max_coalesced_events: 10,
            max_hold_time_us: 8000, // ~half a frame at 60fps
            enabled: true,
        }
    }
}

/// Scroll event coalescer
pub struct ScrollCoalescer {
    config: ScrollCoalescingConfig,
    pending_events: Vec<CoalescedScrollEvent>,
}

impl ScrollCoalescer {
    /// Create a new scroll coalescer with the given configuration
    pub fn new(config: ScrollCoalescingConfig) -> Self {
        Self {
            config,
            pending_events: Vec::with_capacity(4),
        }
    }

    /// Add a scroll event, potentially coalescing with pending events
    pub fn add_event(&mut self, scroll_location: ScrollLocation, cursor: DeviceIntPoint) {
        if !self.config.enabled {
            // Coalescing disabled, add as individual event
            self.pending_events
                .push(CoalescedScrollEvent::new(scroll_location, cursor));
            return;
        }

        let new_event = CoalescedScrollEvent::new(scroll_location, cursor);

        // Try to coalesce with existing pending event at same cursor position
        let coalesced = self.pending_events.iter_mut().any(|existing| {
            if existing.cursor == cursor {
                existing.try_coalesce(&new_event, self.config.max_coalesced_events)
            } else {
                false
            }
        });

        if !coalesced {
            self.pending_events.push(new_event);
        }
    }

    /// Flush all pending events, returning them for processing
    pub fn flush(&mut self) -> Vec<CoalescedScrollEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Flush events that have been pending longer than the hold time
    pub fn flush_expired(&mut self) -> Vec<CoalescedScrollEvent> {
        let now = std::time::Instant::now();
        let max_hold = std::time::Duration::from_micros(self.config.max_hold_time_us);

        let (expired, pending): (Vec<_>, Vec<_>) = self
            .pending_events
            .drain(..)
            .partition(|event| now.duration_since(event.first_event_time) >= max_hold);

        self.pending_events = pending;
        expired
    }

    /// Check if there are any pending events
    pub fn has_pending(&self) -> bool {
        !self.pending_events.is_empty()
    }

    /// Get the number of pending events
    pub fn pending_count(&self) -> usize {
        self.pending_events.len()
    }

    /// Get total event count including coalesced events
    pub fn total_event_count(&self) -> u32 {
        self.pending_events.iter().map(|e| e.event_count).sum()
    }
}

impl Default for ScrollCoalescer {
    fn default() -> Self {
        Self::new(ScrollCoalescingConfig::default())
    }
}

/// Statistics about scroll coalescing effectiveness
#[derive(Clone, Debug, Default)]
pub struct ScrollCoalescingStats {
    /// Total scroll events received
    pub events_received: u64,
    /// Events after coalescing
    pub events_processed: u64,
    /// Events successfully coalesced
    pub events_coalesced: u64,
}

impl ScrollCoalescingStats {
    /// Calculate coalescing ratio (1.0 = no coalescing, higher = more effective)
    pub fn coalescing_ratio(&self) -> f64 {
        if self.events_processed == 0 {
            1.0
        } else {
            self.events_received as f64 / self.events_processed as f64
        }
    }

    /// Record events being flushed
    pub fn record_flush(&mut self, events: &[CoalescedScrollEvent]) {
        for event in events {
            self.events_received += event.event_count as u64;
            self.events_processed += 1;
            if event.event_count > 1 {
                self.events_coalesced += (event.event_count - 1) as u64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cursor(x: i32, y: i32) -> DeviceIntPoint {
        DeviceIntPoint::new(x, y)
    }

    #[test]
    fn test_coalesce_same_cursor() {
        let mut coalescer = ScrollCoalescer::default();
        let cursor = make_cursor(100, 100);

        // Add multiple scroll events at same position
        coalescer.add_event(ScrollLocation::Delta(Point2D::new(0.0, -10.0)), cursor);
        coalescer.add_event(ScrollLocation::Delta(Point2D::new(0.0, -10.0)), cursor);
        coalescer.add_event(ScrollLocation::Delta(Point2D::new(0.0, -10.0)), cursor);

        // Should coalesce into single event
        assert_eq!(coalescer.pending_count(), 1);
        assert_eq!(coalescer.total_event_count(), 3);

        let events = coalescer.flush();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_count, 3);

        // Check accumulated delta
        if let ScrollLocation::Delta(delta) = events[0].scroll_location {
            assert_eq!(delta.y, -30.0);
        } else {
            panic!("Expected Delta scroll location");
        }
    }

    #[test]
    fn test_no_coalesce_different_cursor() {
        let mut coalescer = ScrollCoalescer::default();

        coalescer.add_event(
            ScrollLocation::Delta(Point2D::new(0.0, -10.0)),
            make_cursor(100, 100),
        );
        coalescer.add_event(
            ScrollLocation::Delta(Point2D::new(0.0, -10.0)),
            make_cursor(200, 200),
        );

        // Should not coalesce - different positions
        assert_eq!(coalescer.pending_count(), 2);
    }

    #[test]
    fn test_max_coalesced_events() {
        let config = ScrollCoalescingConfig {
            max_coalesced_events: 3,
            ..Default::default()
        };
        let mut coalescer = ScrollCoalescer::new(config);
        let cursor = make_cursor(100, 100);

        // Add more events than max
        for _ in 0..5 {
            coalescer.add_event(ScrollLocation::Delta(Point2D::new(0.0, -10.0)), cursor);
        }

        // Should have created new event after hitting max
        assert_eq!(coalescer.pending_count(), 2);
    }

    #[test]
    fn test_stats_tracking() {
        let mut stats = ScrollCoalescingStats::default();

        let events = vec![
            CoalescedScrollEvent {
                scroll_location: ScrollLocation::Delta(Point2D::new(0.0, -30.0)),
                cursor: make_cursor(100, 100),
                event_count: 3,
                first_event_time: std::time::Instant::now(),
            },
            CoalescedScrollEvent {
                scroll_location: ScrollLocation::Delta(Point2D::new(0.0, -10.0)),
                cursor: make_cursor(200, 200),
                event_count: 1,
                first_event_time: std::time::Instant::now(),
            },
        ];

        stats.record_flush(&events);

        assert_eq!(stats.events_received, 4);
        assert_eq!(stats.events_processed, 2);
        assert_eq!(stats.events_coalesced, 2);
        assert_eq!(stats.coalescing_ratio(), 2.0);
    }
}

//! Scroll Event Coalescing
//!
//! This module provides scroll event coalescing to reduce layout
//! recalculations during fast scrolling while maintaining responsiveness.
//!
//! Implements A3.2: Scroll event coalescing logic with ScrollLocation awareness.

use std::collections::HashMap;
use euclid::Scale;
use webrender_api::units::DeviceIntPoint;
use webrender_api::ScrollLocation;

/// Maximum number of different cursor positions before forcing a flush
const DEFAULT_MAX_COALESCED_CURSORS: usize = 8;

/// A scroll event that can be coalesced
#[derive(Clone, Copy, Debug)]
pub struct ScrollEvent {
    /// Scroll by this offset, or to Start or End
    pub scroll_location: ScrollLocation,
    /// Apply changes to the frame at this location
    pub cursor: DeviceIntPoint,
    /// The number of OS events coalesced into this one
    pub event_count: u32,
}

impl ScrollEvent {
    /// Create a new scroll event
    pub fn new(scroll_location: ScrollLocation, cursor: DeviceIntPoint) -> Self {
        Self {
            scroll_location,
            cursor,
            event_count: 1,
        }
    }
}
/// Coalesces scroll events by cursor position before adding to pending events.
///
/// Delta events at the same cursor position are combined using weighted averaging.
/// Start/End events trigger immediate flush and are not coalesced.
#[derive(Debug)]
pub struct ScrollCoalescer {
    /// Map from cursor position to accumulated scroll delta
    pending_by_cursor: HashMap<DeviceIntPoint, ScrollEvent>,
    /// Maximum events to coalesce before flushing
    max_coalesced_events: usize,
}

impl Default for ScrollCoalescer {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_COALESCED_CURSORS)
    }
}

impl ScrollCoalescer {
    /// Create a new scroll coalescer with the specified threshold
    pub fn new(max_coalesced_events: usize) -> Self {
        Self {
            pending_by_cursor: HashMap::new(),
            max_coalesced_events,
        }
    }

    /// Add a scroll event, coalescing with existing events at same cursor position.
    ///
    /// Returns Some(events) if the coalescer should be flushed (threshold reached or special event).
    pub fn add_event(&mut self, event: ScrollEvent) -> Option<Vec<ScrollEvent>> {
        match event.scroll_location {
            // Start/End events should flush immediately and not be coalesced
            ScrollLocation::Start | ScrollLocation::End => {
                let mut events = self.flush();
                events.push(event);
                Some(events)
            }
            ScrollLocation::Delta(delta) => {
                match self.pending_by_cursor.get_mut(&event.cursor) {
                    Some(existing) => {
                        // Coalesce: combine deltas using weighted average
                        if let ScrollLocation::Delta(existing_delta) = existing.scroll_location {
                            let new_count = existing.event_count + event.event_count;
                            let old_scale = Scale::<f32, (), ()>::new(existing.event_count as f32);
                            let new_scale = Scale::<f32, (), ()>::new(new_count as f32);

                            // Average the deltas (same logic as process_pending_scroll_events)
                            existing.scroll_location = ScrollLocation::Delta(
                                (existing_delta * old_scale.0 + delta * event.event_count as f32) / new_scale.0
                            );
                            existing.event_count = new_count;
                        }
                    }
                    None => {
                        self.pending_by_cursor.insert(event.cursor, event);
                    }
                }

                // Flush if we've accumulated too many different cursor positions
                if self.pending_by_cursor.len() >= self.max_coalesced_events {
                    Some(self.flush())
                } else {
                    None
                }
            }
        }
            }

    /// Drain all coalesced events
    pub fn flush(&mut self) -> Vec<ScrollEvent> {
        self.pending_by_cursor.drain().map(|(_, event)| event).collect()
    }

    /// Drain coalesced events, returning them for processing
    pub fn drain_coalesced(&mut self) -> Vec<ScrollEvent> {
        self.flush()
    }

    /// Check if there are any pending events
    pub fn has_pending(&self) -> bool {
        !self.pending_by_cursor.is_empty()
    }

    /// Get the number of pending cursor positions
    pub fn pending_count(&self) -> usize {
        self.pending_by_cursor.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webrender_api::units::LayoutVector2D;

    fn make_delta_event(dx: f32, dy: f32, x: i32, y: i32) -> ScrollEvent {
        ScrollEvent::new(
            ScrollLocation::Delta(LayoutVector2D::new(dx, dy)),
            DeviceIntPoint::new(x, y),
        )
    }

    #[test]
    fn test_single_delta_event_no_flush() {
        let mut coalescer = ScrollCoalescer::new(8);
        let result = coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        assert!(result.is_none());
        assert!(coalescer.has_pending());
        assert_eq!(coalescer.pending_count(), 1);
    }

    #[test]
    fn test_coalescing_same_cursor_position() {
        let mut coalescer = ScrollCoalescer::new(8);
        
        // Add 5 events at the same cursor position
        for _ in 0..5 {
            coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        }
        
        // Should only have 1 pending position
        assert_eq!(coalescer.pending_count(), 1);
        
        let events = coalescer.flush();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_count, 5);
        
        // Check delta was averaged: (10+10+10+10+10)/5 = 10
        if let ScrollLocation::Delta(delta) = events[0].scroll_location {
            assert!((delta.y - 10.0).abs() < 0.001);
        } else {
            panic!("Expected Delta");
        }
    }

    #[test]
    fn test_different_cursor_positions_not_coalesced() {
        let mut coalescer = ScrollCoalescer::new(8);
        
        coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        coalescer.add_event(make_delta_event(0.0, 20.0, 200, 200));
        
        assert_eq!(coalescer.pending_count(), 2);
        
        let events = coalescer.flush();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_start_event_triggers_flush() {
        let mut coalescer = ScrollCoalescer::new(8);
        
        // Add a delta event first
        coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        assert_eq!(coalescer.pending_count(), 1);
        
        // Add a Start event - should flush pending and include Start
        let start_event = ScrollEvent::new(ScrollLocation::Start, DeviceIntPoint::new(100, 100));
        let result = coalescer.add_event(start_event);
        
        assert!(result.is_some());
        let events = result.unwrap();
        assert_eq!(events.len(), 2); // 1 pending delta + 1 start
        assert!(matches!(events[1].scroll_location, ScrollLocation::Start));
        
        // Coalescer should be empty now
        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_end_event_triggers_flush() {
        let mut coalescer = ScrollCoalescer::new(8);
        
        coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        
        let end_event = ScrollEvent::new(ScrollLocation::End, DeviceIntPoint::new(100, 100));
        let result = coalescer.add_event(end_event);
        
        assert!(result.is_some());
        let events = result.unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[1].scroll_location, ScrollLocation::End));
    }

    #[test]
    fn test_threshold_flush() {
        let mut coalescer = ScrollCoalescer::new(3); // Low threshold for testing
        
        // Add events at different cursor positions
        coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        coalescer.add_event(make_delta_event(0.0, 10.0, 200, 200));
        
        // Third position should trigger threshold flush
        let result = coalescer.add_event(make_delta_event(0.0, 10.0, 300, 300));
        
        assert!(result.is_some());
        let events = result.unwrap();
        assert_eq!(events.len(), 3);
        assert!(!coalescer.has_pending());
    }

    #[test]
    fn test_event_count_tracking() {
        let mut coalescer = ScrollCoalescer::new(8);
        
        // Manually set event_count to simulate multiple OS events
        let mut event1 = make_delta_event(0.0, 5.0, 100, 100);
        event1.event_count = 2;
        
        let mut event2 = make_delta_event(0.0, 10.0, 100, 100);
        event2.event_count = 3;
        
        coalescer.add_event(event1);
        coalescer.add_event(event2);
        
        let events = coalescer.flush();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_count, 5); // 2 + 3
    }

    #[test]
    fn test_drain_coalesced_alias() {
        let mut coalescer = ScrollCoalescer::new(8);
        coalescer.add_event(make_delta_event(0.0, 10.0, 100, 100));
        
        let events = coalescer.drain_coalesced();
        assert_eq!(events.len(), 1);
        assert!(!coalescer.has_pending());
    }
}

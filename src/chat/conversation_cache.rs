// SPDX-FileCopyrightText: 2026 AprilNEA LLC
//
// SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-Commercial

//! LRU cache for MessageList entities, following Zed's pattern.
//!
//! Each conversation has its own MessageList Entity that maintains all state
//! (messages, scroll position, height cache). Switching conversations means
//! switching which Entity is active, not replacing content within one Entity.

use std::collections::{HashMap, HashSet};

use gpui::*;
use uuid::Uuid;

use super::message_list::MessageList;

/// Default maximum number of cached conversations.
const DEFAULT_MAX_SIZE: usize = 5;

/// LRU cache for conversation MessageList entities.
///
/// This follows Zed's pattern where each buffer/conversation has its own
/// Entity, and switching is done by changing which Entity is active.
pub struct ConversationCache {
    /// conversation_id -> MessageList Entity
    cache: HashMap<Uuid, Entity<MessageList>>,
    /// Maximum number of cached conversations
    max_size: usize,
    /// LRU access order (most recently accessed at the end)
    access_order: Vec<Uuid>,
    /// Conversations with active streams (cannot be evicted)
    active_streams: HashSet<Uuid>,
}

impl ConversationCache {
    /// Create a new conversation cache with default size.
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_SIZE)
    }

    /// Create a new conversation cache with specified max size.
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
            access_order: Vec::new(),
            active_streams: HashSet::new(),
        }
    }

    /// Take an Entity from the cache (removes it from cache).
    ///
    /// Updates LRU order. Returns None if not in cache.
    pub fn take(&mut self, conversation_id: Uuid) -> Option<Entity<MessageList>> {
        if let Some(entity) = self.cache.remove(&conversation_id) {
            // Remove from access order
            self.access_order.retain(|id| *id != conversation_id);
            Some(entity)
        } else {
            None
        }
    }

    /// Get an Entity reference without removing it from cache.
    ///
    /// Updates LRU order.
    #[allow(dead_code)]
    pub fn get(&mut self, conversation_id: Uuid) -> Option<&Entity<MessageList>> {
        if self.cache.contains_key(&conversation_id) {
            // Update access order
            self.access_order.retain(|id| *id != conversation_id);
            self.access_order.push(conversation_id);
            self.cache.get(&conversation_id)
        } else {
            None
        }
    }

    /// Insert an Entity into the cache.
    ///
    /// May trigger LRU eviction if at capacity.
    pub fn insert(&mut self, conversation_id: Uuid, entity: Entity<MessageList>) {
        // If already in cache, update it
        if self.cache.contains_key(&conversation_id) {
            self.access_order.retain(|id| *id != conversation_id);
        } else {
            // Evict if at capacity
            self.evict_if_needed();
        }

        self.cache.insert(conversation_id, entity);
        self.access_order.push(conversation_id);
    }

    /// Remove an Entity from the cache (e.g., when conversation is deleted).
    pub fn remove(&mut self, conversation_id: Uuid) -> Option<Entity<MessageList>> {
        self.access_order.retain(|id| *id != conversation_id);
        self.active_streams.remove(&conversation_id);
        self.cache.remove(&conversation_id)
    }

    /// Mark a conversation as having an active stream.
    ///
    /// Conversations with active streams cannot be evicted.
    #[allow(dead_code)]
    pub fn set_stream_active(&mut self, conversation_id: Uuid, active: bool) {
        if active {
            self.active_streams.insert(conversation_id);
        } else {
            self.active_streams.remove(&conversation_id);
        }
    }

    /// Check if a conversation has an active stream.
    #[allow(dead_code)]
    pub fn has_active_stream(&self, conversation_id: Uuid) -> bool {
        self.active_streams.contains(&conversation_id)
    }

    /// Check if a conversation is in the cache.
    #[allow(dead_code)]
    pub fn contains(&self, conversation_id: Uuid) -> bool {
        self.cache.contains_key(&conversation_id)
    }

    /// Get the number of cached conversations.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if the cache is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all cached conversations.
    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.cache.clear();
        self.access_order.clear();
        self.active_streams.clear();
    }

    /// Evict the oldest conversation if at capacity.
    fn evict_if_needed(&mut self) {
        while self.cache.len() >= self.max_size {
            // Find the oldest conversation that doesn't have an active stream
            let to_evict = self
                .access_order
                .iter()
                .find(|id| !self.active_streams.contains(id))
                .copied();

            if let Some(id) = to_evict {
                tracing::debug!("Evicting conversation {} from cache", id);
                self.cache.remove(&id);
                self.access_order.retain(|i| *i != id);
            } else {
                // All conversations have active streams, can't evict
                tracing::warn!(
                    "Cannot evict any conversation from cache - all have active streams"
                );
                break;
            }
        }
    }
}

impl Default for ConversationCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests would require a GPUI test context to create entities.
    // For now, we just verify the basic logic without entity creation.

    #[test]
    fn test_access_order() {
        let mut cache = ConversationCache::with_max_size(3);

        // Simulate access order tracking without actual entities
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        cache.access_order.push(id1);
        cache.access_order.push(id2);
        cache.access_order.push(id3);

        // Most recent should be at the end
        assert_eq!(cache.access_order.last(), Some(&id3));
    }

    #[test]
    fn test_active_streams() {
        let mut cache = ConversationCache::new();
        let id = Uuid::new_v4();

        assert!(!cache.has_active_stream(id));

        cache.set_stream_active(id, true);
        assert!(cache.has_active_stream(id));

        cache.set_stream_active(id, false);
        assert!(!cache.has_active_stream(id));
    }
}

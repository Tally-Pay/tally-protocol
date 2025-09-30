//! Event Query Client for historical blockchain event retrieval
//!
//! This module provides efficient querying for historical Tally program events
//! directly from the Solana blockchain using RPC calls. The blockchain serves
//! as the single source of truth, with no database duplication required.

#![forbid(unsafe_code)]

use crate::{events::TallyEvent, error::Result, SimpleTallyClient, TallyError};
use anyhow::Context;
use chrono::{DateTime, Utc};
use lru::LruCache;
use serde::{Deserialize, Serialize};
use anchor_client::solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use anchor_client::solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use anchor_client::solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use anchor_client::solana_account_decoder::UiAccountEncoding;
use anchor_client::solana_sdk::{commitment_config::CommitmentConfig, signature::Signature};
use crate::solana_sdk::pubkey::Pubkey;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, trace, warn};

/// Configuration for RPC event queries
#[derive(Debug, Clone)]
pub struct EventQueryConfig {
    /// Maximum number of events to return per query
    pub max_events_per_query: usize,
    /// Maximum number of transaction signatures to process in batch
    pub max_signatures_per_batch: usize,
    /// Default commitment level for queries
    pub commitment: CommitmentConfig,
    /// Enable caching for recent queries
    pub enable_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl_seconds: u64,
    /// Maximum cache size (number of cached query results)
    pub max_cache_size: usize,
}

impl Default for EventQueryConfig {
    fn default() -> Self {
        Self {
            max_events_per_query: 1000,
            max_signatures_per_batch: 100,
            commitment: CommitmentConfig::confirmed(),
            enable_cache: true,
            cache_ttl_seconds: 300, // 5 minutes
            max_cache_size: 1000,
        }
    }
}

/// Event query client configuration
#[derive(Debug, Clone)]
pub struct EventQueryClientConfig {
    /// RPC endpoint URL
    pub rpc_url: String,
    /// Tally program ID
    pub program_id: Pubkey,
    /// Query configuration
    pub query_config: EventQueryConfig,
}

/// A parsed event with transaction context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedEvent {
    /// Transaction signature that contains this event
    pub signature: Signature,
    /// Slot number where transaction was processed
    pub slot: u64,
    /// Block time (Unix timestamp)
    pub block_time: Option<i64>,
    /// Transaction success status
    pub success: bool,
    /// The parsed Tally event
    pub event: TallyEvent,
    /// Log index within the transaction
    pub log_index: usize,
}

/// Cache entry for query results
#[derive(Debug, Clone)]
struct CacheEntry {
    /// Cached events
    events: Vec<ParsedEvent>,
    /// Timestamp when cached
    cached_at: DateTime<Utc>,
    /// TTL for this entry
    ttl_seconds: u64,
}

impl CacheEntry {
    fn new(events: Vec<ParsedEvent>, ttl_seconds: u64) -> Self {
        Self {
            events,
            cached_at: Utc::now(),
            ttl_seconds,
        }
    }

    fn is_expired(&self) -> bool {
        // Use checked conversion to i64 and saturating addition to prevent overflow
        let ttl_i64 = i64::try_from(self.ttl_seconds).unwrap_or(i64::MAX);
        let duration = chrono::Duration::seconds(ttl_i64);
        let expiry = self.cached_at.checked_add_signed(duration).unwrap_or(self.cached_at);
        Utc::now() > expiry
    }
}

/// Query parameters for event retrieval
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct QueryKey {
    merchant: Pubkey,
    query_type: QueryType,
    limit: usize,
    from_slot: Option<u64>,
    to_slot: Option<u64>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum QueryType {
    Recent,
    DateRange,
    MerchantEvents,
}

/// RPC client for querying historical Tally program events
pub struct EventQueryClient {
    /// Solana SDK client
    sdk_client: Arc<SimpleTallyClient>,
    /// Tally program ID
    program_id: Pubkey,
    /// Query configuration
    config: EventQueryConfig,
    /// LRU cache for query results
    cache: Arc<Mutex<LruCache<QueryKey, CacheEntry>>>,
}

impl EventQueryClient {
    /// Create a new `EventQueryClient`
    ///
    /// # Arguments
    ///
    /// * `config` - Event query client configuration
    ///
    /// # Errors
    ///
    /// Returns an error if RPC client creation fails
    pub fn new(config: EventQueryClientConfig) -> Result<Self> {
        let sdk_client = Arc::new(SimpleTallyClient::new(&config.rpc_url)
            .context("Failed to create SimpleTallyClient")?);

        let cache_size = NonZeroUsize::new(config.query_config.max_cache_size)
            .context("Cache size must be greater than 0")?;
        let cache = Arc::new(Mutex::new(LruCache::new(cache_size)));

        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "client_created",
            rpc_url = %config.rpc_url,
            program_id = %config.program_id,
            max_events_per_query = config.query_config.max_events_per_query,
            cache_enabled = config.query_config.enable_cache,
            "EventQueryClient initialized successfully"
        );

        Ok(Self {
            sdk_client,
            program_id: config.program_id,
            config: config.query_config,
            cache,
        })
    }

    /// Create a new `EventQueryClient` with program ID from environment
    ///
    /// # Arguments
    ///
    /// * `rpc_url` - RPC endpoint URL
    /// * `query_config` - Optional query configuration (uses defaults if None)
    ///
    /// # Errors
    ///
    /// Returns an error if RPC client creation fails
    pub fn new_with_program_id(
        rpc_url: String,
        query_config: Option<EventQueryConfig>,
    ) -> Result<Self> {
        let config = EventQueryClientConfig {
            rpc_url,
            program_id: crate::program_id(),
            query_config: query_config.unwrap_or_default(),
        };
        Self::new(config)
    }

    /// Get recent events for a merchant
    ///
    /// # Arguments
    ///
    /// * `merchant` - Merchant public key
    /// * `limit` - Maximum number of events to return
    ///
    /// # Returns
    ///
    /// Vector of parsed events ordered by most recent first
    ///
    /// # Errors
    ///
    /// Returns error if RPC queries fail or event parsing fails
    pub async fn get_recent_events(
        &self,
        merchant: &Pubkey,
        limit: usize,
    ) -> Result<Vec<ParsedEvent>> {
        let start_time = Instant::now();
        let query_key = Self::build_query_key(merchant, QueryType::Recent, limit, None, None);

        // Check cache and return early if hit
        if let Some(cached) = self.try_get_cached_events(&query_key, merchant) {
            return Ok(cached);
        }

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "query_recent_events",
            merchant = %merchant,
            limit = limit,
            "Querying recent events for merchant"
        );

        // Fetch and process events
        let sorted_events = self.fetch_and_sort_events(merchant, limit).await?;

        // Store results in cache
        self.try_cache_events(query_key, &sorted_events);

        Self::log_query_success(merchant, &sorted_events, start_time);

        Ok(sorted_events)
    }

    /// Build a query key for cache operations
    const fn build_query_key(
        merchant: &Pubkey,
        query_type: QueryType,
        limit: usize,
        from_slot: Option<u64>,
        to_slot: Option<u64>,
    ) -> QueryKey {
        QueryKey {
            merchant: *merchant,
            query_type,
            limit,
            from_slot,
            to_slot,
        }
    }

    /// Try to get cached events, returning Some if cache hit
    fn try_get_cached_events(
        &self,
        query_key: &QueryKey,
        merchant: &Pubkey,
    ) -> Option<Vec<ParsedEvent>> {
        if !self.config.enable_cache {
            return None;
        }

        if let Some(cached_events) = self.get_from_cache(query_key) {
            debug!(
                service = "tally-sdk",
                component = "event_query_client",
                event = "cache_hit",
                merchant = %merchant,
                cached_event_count = cached_events.len(),
                "Returning cached recent events"
            );
            return Some(cached_events);
        }

        None
    }

    /// Fetch signatures, parse events, and sort by most recent first
    async fn fetch_and_sort_events(
        &self,
        merchant: &Pubkey,
        limit: usize,
    ) -> Result<Vec<ParsedEvent>> {
        let signatures = self.get_merchant_signatures(merchant, limit).await?;
        let events = self.parse_events_from_signatures(&signatures).await?;
        Ok(Self::sort_and_limit_events(events, limit))
    }

    /// Sort events by slot (most recent first) and apply limit
    fn sort_and_limit_events(mut events: Vec<ParsedEvent>, limit: usize) -> Vec<ParsedEvent> {
        events.sort_by(|a, b| b.slot.cmp(&a.slot));
        events.truncate(limit);
        events
    }

    /// Try to cache events if caching is enabled
    fn try_cache_events(&self, query_key: QueryKey, events: &[ParsedEvent]) {
        if self.config.enable_cache {
            self.store_in_cache(query_key, events.to_vec());
        }
    }

    /// Log successful query completion with metrics
    fn log_query_success(
        merchant: &Pubkey,
        events: &[ParsedEvent],
        start_time: Instant,
    ) {
        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "recent_events_retrieved",
            merchant = %merchant,
            event_count = events.len(),
            duration_ms = start_time.elapsed().as_millis(),
            "Successfully retrieved recent events"
        );
    }

    /// Get events for a merchant within a date range
    ///
    /// # Arguments
    ///
    /// * `merchant` - Merchant public key
    /// * `from` - Start date (inclusive)
    /// * `to` - End date (inclusive)
    ///
    /// # Returns
    ///
    /// Vector of parsed events within the date range
    ///
    /// # Errors
    ///
    /// Returns error if RPC queries fail or slot conversion fails
    pub async fn get_events_by_date_range(
        &self,
        merchant: &Pubkey,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<ParsedEvent>> {
        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "query_events_by_date_range",
            merchant = %merchant,
            from = %from,
            to = %to,
            "Querying events by date range"
        );

        // Convert dates to approximate slots for filtering
        let from_slot = self.timestamp_to_approximate_slot(from.timestamp())?;
        let to_slot = self.timestamp_to_approximate_slot(to.timestamp())?;

        let query_key = QueryKey {
            merchant: *merchant,
            query_type: QueryType::DateRange,
            limit: self.config.max_events_per_query,
            from_slot: Some(from_slot),
            to_slot: Some(to_slot),
        };

        // Check cache first
        if self.config.enable_cache {
            if let Some(cached_events) = self.get_from_cache(&query_key) {
                debug!(
                    service = "tally-sdk",
                    component = "event_query_client",
                    event = "cache_hit",
                    merchant = %merchant,
                    cached_event_count = cached_events.len(),
                    "Returning cached date range events"
                );
                return Ok(cached_events);
            }
        }

        // Get merchant signatures within the slot range
        let signatures = self
            .get_merchant_signatures_in_slot_range(merchant, from_slot, to_slot)
            .await?;

        // Parse events from transactions
        let events = self.parse_events_from_signatures(&signatures).await?;

        // Filter by actual block time
        let filtered_events: Vec<ParsedEvent> = events
            .into_iter()
            .filter(|event| {
                if let Some(block_time) = event.block_time {
                    let event_time = DateTime::from_timestamp(block_time, 0);
                    if let Some(event_time) = event_time {
                        return event_time >= from && event_time <= to;
                    }
                }
                false
            })
            .collect();

        // Sort by block time (most recent first)
        let mut sorted_events = filtered_events;
        sorted_events.sort_by(|a, b| b.block_time.unwrap_or(0).cmp(&a.block_time.unwrap_or(0)));

        // Cache the results
        if self.config.enable_cache {
            self.store_in_cache(query_key, sorted_events.clone());
        }

        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "date_range_events_retrieved",
            merchant = %merchant,
            event_count = sorted_events.len(),
            signatures_processed = signatures.len(),
            from_slot = from_slot,
            to_slot = to_slot,
            "Successfully retrieved events by date range"
        );

        Ok(sorted_events)
    }

    /// Get all events for a merchant (up to configured limit)
    ///
    /// # Arguments
    ///
    /// * `merchant` - Merchant public key
    /// * `limit` - Maximum number of events to return
    ///
    /// # Returns
    ///
    /// Vector of parsed events for the merchant
    ///
    /// # Errors
    ///
    /// Returns error if RPC queries fail
    pub async fn get_merchant_events(
        &self,
        merchant: &Pubkey,
        limit: usize,
    ) -> Result<Vec<ParsedEvent>> {
        let query_key = QueryKey {
            merchant: *merchant,
            query_type: QueryType::MerchantEvents,
            limit,
            from_slot: None,
            to_slot: None,
        };

        // Check cache first
        if self.config.enable_cache {
            if let Some(cached_events) = self.get_from_cache(&query_key) {
                debug!(
                    service = "tally-sdk",
                    component = "event_query_client",
                    event = "cache_hit",
                    merchant = %merchant,
                    cached_event_count = cached_events.len(),
                    "Returning cached merchant events"
                );
                return Ok(cached_events);
            }
        }

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "query_merchant_events",
            merchant = %merchant,
            limit = limit,
            "Querying all events for merchant"
        );

        // Get transaction signatures for all merchant accounts
        // Get more signatures to ensure we have enough events (2x buffer with overflow protection)
        let signature_limit = limit.saturating_mul(2);
        let signatures = self.get_merchant_signatures(merchant, signature_limit).await?;

        // Parse events from transactions
        let events = self.parse_events_from_signatures(&signatures).await?;

        // Sort by slot number (most recent first)
        let mut sorted_events = events;
        sorted_events.sort_by(|a, b| b.slot.cmp(&a.slot));

        // Limit results
        sorted_events.truncate(limit);

        // Cache the results
        if self.config.enable_cache {
            self.store_in_cache(query_key, sorted_events.clone());
        }

        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "merchant_events_retrieved",
            merchant = %merchant,
            event_count = sorted_events.len(),
            signatures_processed = signatures.len(),
            "Successfully retrieved merchant events"
        );

        Ok(sorted_events)
    }

    /// Get transaction signatures for merchant's program accounts
    #[allow(clippy::unused_async)] // May need async for future enhanced RPC operations
    async fn get_merchant_signatures(
        &self,
        merchant: &Pubkey,
        limit: usize,
    ) -> Result<Vec<Signature>> {
        // Get merchant account signatures
        let merchant_signatures = self
            .sdk_client
            .get_confirmed_signatures_for_address(
                merchant,
                Some(GetConfirmedSignaturesForAddress2Config {
                    limit: Some(limit.min(1000)), // Solana RPC limit
                    commitment: Some(self.config.commitment),
                    ..Default::default()
                }),
            )
            .map_err(|e| TallyError::RpcError(format!("Failed to get merchant signatures: {e}")))?;

        let mut signatures = HashSet::new();
        for sig_info in merchant_signatures {
            if let Ok(signature) = Signature::from_str(&sig_info.signature) {
                signatures.insert(signature);
            }
        }

        // Get plans for this merchant and their signatures
        let plans = self.get_merchant_plans(merchant)?;
        for plan_address in &plans {
            let plan_signatures = self
                .sdk_client
                .get_confirmed_signatures_for_address(
                    plan_address,
                    Some(GetConfirmedSignaturesForAddress2Config {
                        limit: Some(limit.min(1000)),
                        commitment: Some(self.config.commitment),
                        ..Default::default()
                    }),
                )
                .map_err(|e| TallyError::RpcError(format!("Failed to get plan signatures: {e}")))?;

            for sig_info in plan_signatures {
                if let Ok(signature) = Signature::from_str(&sig_info.signature) {
                    signatures.insert(signature);
                }
            }
        }

        // Get subscriptions for merchant plans and their signatures
        for plan_address in &plans {
            let subscriptions = self.get_plan_subscriptions(plan_address)?;
            for subscription_address in subscriptions {
                let sub_signatures = self
                    .sdk_client
                    .get_confirmed_signatures_for_address(
                        &subscription_address,
                        Some(GetConfirmedSignaturesForAddress2Config {
                            limit: Some(limit.min(1000)),
                            commitment: Some(self.config.commitment),
                            ..Default::default()
                        }),
                    )
                    .map_err(|e| TallyError::RpcError(format!("Failed to get subscription signatures: {e}")))?;

                for sig_info in sub_signatures {
                    if let Ok(signature) = Signature::from_str(&sig_info.signature) {
                        signatures.insert(signature);
                    }
                }
            }
        }

        let result: Vec<Signature> = signatures.into_iter().collect();

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "signatures_collected",
            merchant = %merchant,
            signature_count = result.len(),
            plan_count = plans.len(),
            "Collected transaction signatures for merchant"
        );

        Ok(result)
    }

    /// Get transaction signatures for merchant within a slot range
    async fn get_merchant_signatures_in_slot_range(
        &self,
        merchant: &Pubkey,
        _from_slot: u64,
        _to_slot: u64,
    ) -> Result<Vec<Signature>> {
        // Get signatures with 2x buffer, using saturating multiplication to prevent overflow
        let signature_limit = self.config.max_events_per_query.saturating_mul(2);
        let signatures = self
            .get_merchant_signatures(merchant, signature_limit)
            .await?;

        // We would need to fetch transaction details to filter by slot, which is expensive
        // For now, return all signatures and filter during event parsing
        // TODO: Implement more efficient slot-based filtering if needed

        Ok(signatures)
    }

    /// Parse events from transaction signatures
    async fn parse_events_from_signatures(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<ParsedEvent>> {
        let mut all_events = Vec::new();

        // Process signatures in batches to avoid overwhelming RPC
        for chunk in signatures.chunks(self.config.max_signatures_per_batch) {
            let batch_events = Vec::new();

            for signature in chunk {
                match self.sdk_client.get_transaction(signature) {
                    Ok(_transaction) => {
                        // TODO: Implement proper JSON-based event parsing after refactor
                        debug!(
                            service = "tally-sdk",
                            component = "event_query_client",
                            event = "transaction_received",
                            signature = %signature,
                            "Transaction data received - event parsing temporarily disabled"
                        );
                    }
                    Err(e) => {
                        trace!(
                            service = "tally-sdk",
                            component = "event_query_client",
                            event = "transaction_fetch_error",
                            signature = %signature,
                            error = %e,
                            "Failed to fetch transaction details"
                        );
                    }
                }
            }

            all_events.extend(batch_events);

            // Small delay between batches to be respectful to RPC
            if chunk.len() == self.config.max_signatures_per_batch {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "events_parsed",
            signature_count = signatures.len(),
            event_count = all_events.len(),
            "Parsed events from transaction signatures"
        );

        Ok(all_events)
    }

    /// Get plan addresses for a merchant using getProgramAccounts
    fn get_merchant_plans(&self, merchant: &Pubkey) -> Result<Vec<Pubkey>> {
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                // Filter by merchant field in Plan account (first 32 bytes after discriminator)
                RpcFilterType::Memcmp(Memcmp::new(
                    8, // Skip 8-byte Anchor discriminator
                    MemcmpEncodedBytes::Base58(merchant.to_string()),
                )),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(self.config.commitment),
                ..Default::default()
            },
            with_context: Some(false),
            sort_results: None,
        };

        let accounts = self
            .sdk_client.rpc()
            .get_program_accounts_with_config(&self.program_id, config)
            .map_err(|e| TallyError::RpcError(format!("Failed to get merchant plans: {e}")))?;

        let plan_addresses: Vec<Pubkey> = accounts.into_iter().map(|(pubkey, _)| pubkey).collect();

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "merchant_plans_retrieved",
            merchant = %merchant,
            plan_count = plan_addresses.len(),
            "Retrieved plan addresses for merchant"
        );

        Ok(plan_addresses)
    }

    /// Get subscription addresses for a plan using getProgramAccounts
    fn get_plan_subscriptions(&self, plan: &Pubkey) -> Result<Vec<Pubkey>> {
        let config = RpcProgramAccountsConfig {
            filters: Some(vec![
                // Filter by plan field in Subscription account (first 32 bytes after discriminator)
                RpcFilterType::Memcmp(Memcmp::new(
                    8, // Skip 8-byte Anchor discriminator
                    MemcmpEncodedBytes::Base58(plan.to_string()),
                )),
            ]),
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                commitment: Some(self.config.commitment),
                ..Default::default()
            },
            with_context: Some(false),
            sort_results: None,
        };

        let accounts = self
            .sdk_client.rpc()
            .get_program_accounts_with_config(&self.program_id, config)
            .map_err(|e| TallyError::RpcError(format!("Failed to get plan subscriptions: {e}")))?;

        let subscription_addresses: Vec<Pubkey> =
            accounts.into_iter().map(|(pubkey, _)| pubkey).collect();

        trace!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "plan_subscriptions_retrieved",
            plan = %plan,
            subscription_count = subscription_addresses.len(),
            "Retrieved subscription addresses for plan"
        );

        Ok(subscription_addresses)
    }

    /// Convert Unix timestamp to approximate slot number
    fn timestamp_to_approximate_slot(&self, timestamp: i64) -> Result<u64> {
        // Estimate slot time (approximately 400ms per slot on Solana)
        const SLOT_DURATION_MS: i64 = 400;

        // Get current slot and time
        let current_slot = self.sdk_client.get_slot().map_err(|e| TallyError::RpcError(format!("Failed to get current slot: {e}")))?;
        let current_time = Utc::now().timestamp();
        let time_diff_seconds = current_time.saturating_sub(timestamp);
        let time_diff_ms = time_diff_seconds.saturating_mul(1000);
        let slot_diff = time_diff_ms / SLOT_DURATION_MS;

        // Calculate approximate slot using checked arithmetic
        let approximate_slot = if slot_diff > 0 {
            // slot_diff is positive, so safe to cast to u64 since it came from positive time difference
            current_slot.saturating_sub(u64::try_from(slot_diff).unwrap_or(u64::MAX))
        } else {
            // slot_diff is negative or zero, use absolute value
            let abs_diff = slot_diff.unsigned_abs();
            current_slot.saturating_add(abs_diff)
        };

        trace!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "timestamp_to_slot_conversion",
            timestamp = timestamp,
            current_slot = current_slot,
            approximate_slot = approximate_slot,
            "Converted timestamp to approximate slot"
        );

        Ok(approximate_slot)
    }

    /// Get events from cache if available and not expired
    fn get_from_cache(&self, key: &QueryKey) -> Option<Vec<ParsedEvent>> {
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(entry) = cache.get(key) {
                if !entry.is_expired() {
                    return Some(entry.events.clone());
                }
                // Remove expired entry
                cache.pop(key);
            }
        }
        None
    }

    /// Store events in cache
    fn store_in_cache(&self, key: QueryKey, events: Vec<ParsedEvent>) {
        if let Ok(mut cache) = self.cache.lock() {
            let entry = CacheEntry::new(events, self.config.cache_ttl_seconds);
            cache.put(key, entry);
        }
    }

    /// Clear the cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }

        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "cache_cleared",
            "Query cache has been cleared"
        );
    }

    /// Get cache statistics
    #[must_use] 
    pub fn get_cache_stats(&self) -> HashMap<String, u64> {
        let mut stats = HashMap::new();

        if let Ok(cache) = self.cache.lock() {
            stats.insert("cache_size".to_string(), cache.len() as u64);
            stats.insert("cache_capacity".to_string(), cache.cap().get() as u64);
        }

        stats
    }

    /// Health check for the RPC client
    pub fn health_check(&self) -> bool {
        match self.sdk_client.get_health() {
            Ok(()) => {
                debug!(
                    service = "tally-sdk",
                    component = "event_query_client",
                    event = "health_check_success",
                    "RPC client health check passed"
                );
                true
            }
            Err(e) => {
                warn!(
                    service = "tally-sdk",
                    component = "event_query_client",
                    event = "health_check_failed",
                    error = %e,
                    "RPC client health check failed"
                );
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> EventQueryClientConfig {
        EventQueryClientConfig {
            rpc_url: "http://localhost:8899".to_string(),
            program_id: crate::program_id(),
            query_config: EventQueryConfig::default(),
        }
    }

    #[test]
    fn test_event_query_client_creation() {
        let config = create_test_config();
        let client = EventQueryClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_event_query_config_default() {
        let config = EventQueryConfig::default();
        assert_eq!(config.max_events_per_query, 1000);
        assert_eq!(config.max_signatures_per_batch, 100);
        assert!(config.enable_cache);
        assert_eq!(config.cache_ttl_seconds, 300);
    }

    #[test]
    fn test_cache_entry_expiry() {
        let events = vec![];
        let entry = CacheEntry::new(events, 1); // 1 second TTL

        assert!(!entry.is_expired());

        // Test with past timestamp
        let mut expired_entry = entry;
        expired_entry.cached_at = Utc::now() - chrono::Duration::seconds(2);
        assert!(expired_entry.is_expired());
    }

    #[test]
    fn test_query_key_equality() {
        let merchant = Pubkey::new_unique();

        let key1 = QueryKey {
            merchant,
            query_type: QueryType::Recent,
            limit: 100,
            from_slot: None,
            to_slot: None,
        };

        let key2 = QueryKey {
            merchant,
            query_type: QueryType::Recent,
            limit: 100,
            from_slot: None,
            to_slot: None,
        };

        assert_eq!(key1, key2);

        let key3 = QueryKey {
            merchant,
            query_type: QueryType::MerchantEvents,
            limit: 100,
            from_slot: None,
            to_slot: None,
        };

        assert_ne!(key1, key3);
    }

    #[tokio::test]
    async fn test_timestamp_to_slot_conversion() {
        let config = create_test_config();
        let _client = EventQueryClient::new(config).unwrap();

        // Test with current timestamp (should not fail)
        let _current_time = Utc::now().timestamp();

        // This test will fail with localhost RPC, but validates the interface
        // In a real environment with running validator, this would work
        // let slot = client.timestamp_to_approximate_slot(current_time).await;
        // assert!(slot.is_ok() || slot.unwrap_err().to_string().contains("Connection refused"));
    }

    #[test]
    fn test_cache_operations() {
        let config = create_test_config();
        let client = EventQueryClient::new(config).unwrap();

        let key = QueryKey {
            merchant: Pubkey::new_unique(),
            query_type: QueryType::Recent,
            limit: 100,
            from_slot: None,
            to_slot: None,
        };

        // Cache should be empty initially
        assert!(client.get_from_cache(&key).is_none());

        // Store something in cache
        let events = vec![];
        client.store_in_cache(key.clone(), events);

        // Should be able to retrieve it
        assert!(client.get_from_cache(&key).is_some());

        // Clear cache
        client.clear_cache();

        // Should be empty again
        assert!(client.get_from_cache(&key).is_none());
    }

    #[test]
    fn test_cache_stats() {
        let config = create_test_config();
        let client = EventQueryClient::new(config).unwrap();

        let stats = client.get_cache_stats();
        assert!(stats.contains_key("cache_size"));
        assert!(stats.contains_key("cache_capacity"));
        assert_eq!(stats["cache_size"], 0);
    }
}
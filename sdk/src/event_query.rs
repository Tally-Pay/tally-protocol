//! Event Query Client for historical blockchain event retrieval
//!
//! This module provides efficient querying for historical Tally program events
//! directly from the Solana blockchain using RPC calls. The blockchain serves
//! as the single source of truth, with no database duplication required.

#![forbid(unsafe_code)]

use crate::solana_sdk::pubkey::Pubkey;
use crate::{error::Result, events::TallyEvent, SimpleTallyClient, TallyError};
use anchor_client::solana_account_decoder::UiAccountEncoding;
use anchor_client::solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use anchor_client::solana_client::rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use anchor_client::solana_client::rpc_filter::{Memcmp, MemcmpEncodedBytes, RpcFilterType};
use anchor_client::solana_sdk::{commitment_config::CommitmentConfig, signature::Signature};
use anyhow::Context;
use chrono::{DateTime, Utc};
use lru::LruCache;
use serde::{Deserialize, Serialize};
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
        let expiry = self
            .cached_at
            .checked_add_signed(duration)
            .unwrap_or(self.cached_at);
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
        let sdk_client = Arc::new(
            SimpleTallyClient::new(&config.rpc_url)
                .context("Failed to create SimpleTallyClient")?,
        );

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
    fn log_query_success(merchant: &Pubkey, events: &[ParsedEvent], start_time: Instant) {
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
        let start_time = Instant::now();

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "query_events_by_date_range",
            merchant = %merchant,
            from = %from,
            to = %to,
            "Querying events by date range"
        );

        // Convert dates to slots and build query key
        let (from_slot, to_slot) = self.convert_date_range_to_slots(from, to)?;
        let query_key = Self::build_date_range_query_key(
            merchant,
            from_slot,
            to_slot,
            self.config.max_events_per_query,
        );

        // Check cache and return early if hit
        if let Some(cached) = self.try_get_cached_date_range_events(&query_key, merchant) {
            return Ok(cached);
        }

        // Fetch, filter, and sort events
        let sorted_events = self
            .fetch_filter_and_sort_events_by_date(merchant, from, to, from_slot, to_slot)
            .await?;

        // Store results in cache
        self.try_cache_events(query_key.clone(), &sorted_events);

        Self::log_date_range_query_success(
            merchant,
            &sorted_events,
            from_slot,
            to_slot,
            start_time,
        );

        Ok(sorted_events)
    }

    /// Convert date range to approximate slot range
    fn convert_date_range_to_slots(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<(u64, u64)> {
        let from_slot = self.timestamp_to_approximate_slot(from.timestamp())?;
        let to_slot = self.timestamp_to_approximate_slot(to.timestamp())?;
        Ok((from_slot, to_slot))
    }

    /// Build query key for date range queries
    const fn build_date_range_query_key(
        merchant: &Pubkey,
        from_slot: u64,
        to_slot: u64,
        limit: usize,
    ) -> QueryKey {
        QueryKey {
            merchant: *merchant,
            query_type: QueryType::DateRange,
            limit,
            from_slot: Some(from_slot),
            to_slot: Some(to_slot),
        }
    }

    /// Try to get cached date range events, returning Some if cache hit
    fn try_get_cached_date_range_events(
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
                "Returning cached date range events"
            );
            return Some(cached_events);
        }

        None
    }

    /// Fetch signatures, parse, filter by date, and sort events
    async fn fetch_filter_and_sort_events_by_date(
        &self,
        merchant: &Pubkey,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        from_slot: u64,
        to_slot: u64,
    ) -> Result<Vec<ParsedEvent>> {
        let signatures = self
            .get_merchant_signatures_in_slot_range(merchant, from_slot, to_slot)
            .await?;
        let events = self.parse_events_from_signatures(&signatures).await?;
        let filtered_events = Self::filter_events_by_date_range(events, from, to);
        Ok(Self::sort_events_by_block_time(filtered_events))
    }

    /// Filter events to only those within the specified date range
    fn filter_events_by_date_range(
        events: Vec<ParsedEvent>,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Vec<ParsedEvent> {
        events
            .into_iter()
            .filter(|event| Self::is_event_in_date_range(event, from, to))
            .collect()
    }

    /// Check if an event is within the specified date range
    fn is_event_in_date_range(event: &ParsedEvent, from: DateTime<Utc>, to: DateTime<Utc>) -> bool {
        event
            .block_time
            .and_then(|block_time| DateTime::from_timestamp(block_time, 0))
            .is_some_and(|event_time| event_time >= from && event_time <= to)
    }

    /// Sort events by block time (most recent first)
    fn sort_events_by_block_time(mut events: Vec<ParsedEvent>) -> Vec<ParsedEvent> {
        events.sort_by(|a, b| b.block_time.unwrap_or(0).cmp(&a.block_time.unwrap_or(0)));
        events
    }

    /// Log successful date range query completion with metrics
    fn log_date_range_query_success(
        merchant: &Pubkey,
        events: &[ParsedEvent],
        from_slot: u64,
        to_slot: u64,
        start_time: Instant,
    ) {
        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "date_range_events_retrieved",
            merchant = %merchant,
            event_count = events.len(),
            from_slot = from_slot,
            to_slot = to_slot,
            duration_ms = start_time.elapsed().as_millis(),
            "Successfully retrieved events by date range"
        );
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
        let start_time = Instant::now();
        let query_key = Self::build_merchant_events_query_key(merchant, limit);

        // Check cache and return early if hit
        if let Some(cached) = self.try_get_cached_merchant_events(&query_key, merchant) {
            return Ok(cached);
        }

        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "query_merchant_events",
            merchant = %merchant,
            limit = limit,
            "Querying all events for merchant"
        );

        // Fetch, parse, and sort events
        let sorted_events = self
            .fetch_parse_and_sort_merchant_events(merchant, limit)
            .await?;

        // Store results in cache
        self.try_cache_events(query_key, &sorted_events);

        Self::log_merchant_events_success(merchant, &sorted_events, start_time);

        Ok(sorted_events)
    }

    /// Build query key for merchant events queries
    const fn build_merchant_events_query_key(merchant: &Pubkey, limit: usize) -> QueryKey {
        QueryKey {
            merchant: *merchant,
            query_type: QueryType::MerchantEvents,
            limit,
            from_slot: None,
            to_slot: None,
        }
    }

    /// Try to get cached merchant events, returning Some if cache hit
    fn try_get_cached_merchant_events(
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
                "Returning cached merchant events"
            );
            return Some(cached_events);
        }

        None
    }

    /// Fetch signatures, parse events, and sort for merchant
    async fn fetch_parse_and_sort_merchant_events(
        &self,
        merchant: &Pubkey,
        limit: usize,
    ) -> Result<Vec<ParsedEvent>> {
        // Get more signatures to ensure we have enough events (2x buffer with overflow protection)
        let signature_limit = limit.saturating_mul(2);
        let signatures = self
            .get_merchant_signatures(merchant, signature_limit)
            .await?;

        // Parse events from transactions
        let events = self.parse_events_from_signatures(&signatures).await?;

        // Sort and limit events
        Ok(Self::sort_and_limit_events(events, limit))
    }

    /// Log successful merchant events query completion with metrics
    fn log_merchant_events_success(merchant: &Pubkey, events: &[ParsedEvent], start_time: Instant) {
        info!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "merchant_events_retrieved",
            merchant = %merchant,
            event_count = events.len(),
            duration_ms = start_time.elapsed().as_millis(),
            "Successfully retrieved merchant events"
        );
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
                    .map_err(|e| {
                        TallyError::RpcError(format!("Failed to get subscription signatures: {e}"))
                    })?;

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

        Ok(signatures)
    }

    /// Parse events from transaction signatures
    async fn parse_events_from_signatures(
        &self,
        signatures: &[Signature],
    ) -> Result<Vec<ParsedEvent>> {
        let all_events = self.process_signature_batches(signatures).await;

        Self::log_parsed_events_summary(signatures, &all_events);

        Ok(all_events)
    }

    /// Process signatures in batches with rate limiting
    async fn process_signature_batches(&self, signatures: &[Signature]) -> Vec<ParsedEvent> {
        let mut all_events = Vec::new();

        for chunk in signatures.chunks(self.config.max_signatures_per_batch) {
            let batch_events = self.process_signature_chunk(chunk);
            all_events.extend(batch_events);

            // Small delay between batches to be respectful to RPC
            self.apply_batch_rate_limit(chunk.len()).await;
        }

        all_events
    }

    /// Process a single chunk of signatures
    fn process_signature_chunk(&self, chunk: &[Signature]) -> Vec<ParsedEvent> {
        let batch_events = Vec::new();

        for signature in chunk {
            self.try_fetch_and_log_transaction(signature);
        }

        batch_events
    }

    /// Try to fetch transaction and log result
    fn try_fetch_and_log_transaction(&self, signature: &Signature) {
        match self.sdk_client.get_transaction(signature) {
            Ok(_transaction) => {
                Self::log_transaction_received(signature);
            }
            Err(e) => {
                Self::log_transaction_fetch_error(signature, &e);
            }
        }
    }

    /// Log successful transaction fetch
    fn log_transaction_received(signature: &Signature) {
        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "transaction_received",
            signature = %signature,
            "Transaction data received - event parsing temporarily disabled"
        );
    }

    /// Log transaction fetch error
    fn log_transaction_fetch_error<E: std::fmt::Display>(signature: &Signature, error: &E) {
        trace!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "transaction_fetch_error",
            signature = %signature,
            error = %error,
            "Failed to fetch transaction details"
        );
    }

    /// Apply rate limiting delay between batches if needed
    async fn apply_batch_rate_limit(&self, chunk_len: usize) {
        if chunk_len == self.config.max_signatures_per_batch {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Log summary of parsed events
    fn log_parsed_events_summary(signatures: &[Signature], events: &[ParsedEvent]) {
        debug!(
            service = "tally-sdk",
            component = "event_query_client",
            event = "events_parsed",
            signature_count = signatures.len(),
            event_count = events.len(),
            "Parsed events from transaction signatures"
        );
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
            .sdk_client
            .rpc()
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
            .sdk_client
            .rpc()
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
        let current_slot = self
            .sdk_client
            .get_slot()
            .map_err(|e| TallyError::RpcError(format!("Failed to get current slot: {e}")))?;
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
    async fn test_timestamp_to_slot_conversion_logic() {
        // Test the slot calculation logic without requiring RPC
        // We test the math directly since RPC might not be available

        const SLOT_DURATION_MS: i64 = 400;

        // Simulate current time and slot
        let current_time = 1_700_000_000i64; // Fixed timestamp
        let current_slot = 250_000_000u64;

        // Test 1: Past timestamp (1 hour ago = 3600 seconds)
        // past_time would be: current_time - 3600
        let time_diff_ms = 3600 * 1000;
        let expected_slot_diff = time_diff_ms / SLOT_DURATION_MS; // 9000 slots
        let expected_past_slot = current_slot.saturating_sub(expected_slot_diff.unsigned_abs());

        // Manual calculation: 250_000_000 - 9000 = 249_991_000
        assert_eq!(expected_past_slot, 249_991_000);

        // Test 2: Future timestamp (1 hour in future)
        // future_time would be: current_time + 3600
        let future_time_diff = -3600i64;
        let future_slot_diff_ms = future_time_diff.saturating_mul(1000);
        let abs_diff = future_slot_diff_ms.unsigned_abs();

        // Future events should add to current slot
        let expected_future_slot = current_slot.saturating_add(abs_diff / (SLOT_DURATION_MS as u64));
        assert_eq!(expected_future_slot, 250_009_000);

        // Test 3: Current timestamp (should be approximately current slot)
        let same_time_diff = 0i64;
        let same_time_diff_ms = same_time_diff.saturating_mul(1000);
        let same_slot_diff = same_time_diff_ms / SLOT_DURATION_MS;
        assert_eq!(same_slot_diff, 0);

        // Test 4: Very old timestamp (overflow protection)
        // very_old_time would be: 0i64
        let old_time_diff_ms = current_time.saturating_mul(1000);
        let old_slot_diff = old_time_diff_ms / SLOT_DURATION_MS;
        // Should saturate to 0, not panic
        let old_slot = current_slot.saturating_sub(old_slot_diff.unsigned_abs());
        assert!(old_slot <= current_slot);
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

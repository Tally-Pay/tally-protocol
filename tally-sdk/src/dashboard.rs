//! Dashboard client for merchant management and analytics

#![forbid(unsafe_code)]
#![allow(clippy::arithmetic_side_effects)] // Safe for business logic calculations
#![allow(clippy::cast_possible_truncation)] // Controlled truncation for display formatting
#![allow(clippy::cast_lossless)] // Safe casting for USDC formatting
#![allow(clippy::cast_precision_loss)] // Controlled precision loss for display formatting

use crate::{
    dashboard_types::{
        DashboardEvent, DashboardSubscription, EventStream, Overview, PlanAnalytics,
    },
    error::{Result, TallyError},
    program_types::{CreatePlanArgs, InitMerchantArgs, Merchant, Plan},
    simple_client::SimpleTallyClient,
    validation::validate_platform_fee_bps,
};
use chrono::Utc;
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use std::collections::HashMap;

/// Dashboard client for merchant management and analytics
///
/// Provides high-level methods for dashboard operations including merchant provisioning,
/// live data fetching, and real-time event monitoring.
pub struct DashboardClient {
    /// Underlying simple client for blockchain operations
    client: SimpleTallyClient,
}

impl DashboardClient {
    /// Create a new dashboard client
    ///
    /// # Arguments
    /// * `cluster_url` - RPC endpoint URL
    ///
    /// # Returns
    /// * `Ok(DashboardClient)` - The dashboard client instance
    ///
    /// # Errors
    /// Returns an error if the underlying client cannot be created
    pub fn new(cluster_url: &str) -> Result<Self> {
        let client = SimpleTallyClient::new(cluster_url)?;
        Ok(Self { client })
    }

    /// Get the underlying simple client
    #[must_use]
    pub const fn client(&self) -> &SimpleTallyClient {
        &self.client
    }

    /// Get the program ID
    #[must_use]
    pub const fn program_id(&self) -> Pubkey {
        self.client.program_id()
    }

    // ========================================
    // Merchant Provisioning Methods
    // ========================================

    /// Provision a new merchant account
    ///
    /// This is a high-level method that checks if the merchant already exists
    /// and creates it if needed. Returns the merchant PDA and transaction signature.
    ///
    /// # Arguments
    /// * `authority` - The merchant's authority keypair
    /// * `merchant_args` - Merchant initialization arguments
    ///
    /// # Returns
    /// * `Ok((Pubkey, String))` - Merchant PDA and transaction signature
    ///
    /// # Errors
    /// Returns an error if merchant creation fails or arguments are invalid
    pub fn provision_merchant<T: Signer>(
        &self,
        authority: &T,
        merchant_args: &InitMerchantArgs,
    ) -> Result<(Pubkey, String)> {
        // Validate platform fee
        validate_platform_fee_bps(merchant_args.platform_fee_bps)?;

        // Check if merchant already exists
        let merchant_pda = self.client.merchant_address(&authority.pubkey());
        if self.client.account_exists(&merchant_pda)? {
            return Err(TallyError::Generic(format!(
                "Merchant account already exists at address: {merchant_pda}"
            )));
        }

        // Create the merchant
        self.client.create_merchant(
            authority,
            &merchant_args.usdc_mint,
            &merchant_args.treasury_ata,
            merchant_args.platform_fee_bps,
        )
    }

    /// Get existing merchant or return None if not found
    ///
    /// # Arguments
    /// * `authority` - The merchant's authority pubkey
    ///
    /// # Returns
    /// * `Ok(Some((Pubkey, Merchant)))` - Merchant PDA and data if found
    /// * `Ok(None)` - If merchant doesn't exist
    ///
    /// # Errors
    /// Returns an error if RPC calls fail
    pub fn get_merchant(&self, authority: &Pubkey) -> Result<Option<(Pubkey, Merchant)>> {
        let merchant_pda = self.client.merchant_address(authority);

        self.client
            .get_merchant(&merchant_pda)?
            .map_or_else(|| Ok(None), |merchant| Ok(Some((merchant_pda, merchant))))
    }

    /// Create a new subscription plan for a merchant
    ///
    /// # Arguments
    /// * `authority` - The merchant's authority keypair
    /// * `plan_args` - Plan creation arguments
    ///
    /// # Returns
    /// * `Ok((Pubkey, String))` - Plan PDA and transaction signature
    ///
    /// # Errors
    /// Returns an error if plan creation fails or arguments are invalid
    pub fn create_plan<T: Signer>(
        &self,
        authority: &T,
        plan_args: CreatePlanArgs,
    ) -> Result<(Pubkey, String)> {
        // Delegate to the underlying client's create_plan method
        self.client.create_plan(authority, plan_args)
    }

    // ========================================
    // Live Data Fetching Methods
    // ========================================

    /// Get comprehensive overview statistics for a merchant
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    ///
    /// # Returns
    /// * `Ok(Overview)` - Overview statistics
    ///
    /// # Errors
    /// Returns an error if the merchant doesn't exist or data fetching fails
    pub fn get_merchant_overview(&self, merchant: &Pubkey) -> Result<Overview> {
        // Get merchant data
        let merchant_data = self.client.get_merchant(merchant)?.ok_or_else(|| {
            TallyError::AccountNotFound(format!("Merchant not found: {merchant}"))
        })?;

        // Get all plans for this merchant
        let plans = self.client.list_plans(merchant)?;
        let total_plans = u32::try_from(plans.len())
            .map_err(|_| TallyError::Generic("Too many plans for merchant".to_string()))?;

        // Collect all subscription data across all plans
        let mut all_subscriptions = Vec::new();
        for (plan_address, _plan) in &plans {
            let subs = self.client.list_subscriptions(plan_address)?;
            all_subscriptions.extend(subs);
        }

        // Calculate statistics
        let current_time = Utc::now().timestamp();
        let month_start = current_time.saturating_sub(30 * 24 * 60 * 60); // 30 days ago

        let mut active_count = 0u32;
        let mut inactive_count = 0u32;
        let mut total_revenue = 0u64;
        let mut monthly_revenue = 0u64;
        let mut monthly_new_subs = 0u32;
        let mut monthly_canceled_subs = 0u32;

        for (_sub_address, subscription) in &all_subscriptions {
            if subscription.active {
                active_count = active_count.saturating_add(1);
            } else {
                inactive_count = inactive_count.saturating_add(1);
            }

            // Calculate revenue (renewals * last_amount)
            let sub_revenue =
                u64::from(subscription.renewals).saturating_mul(subscription.last_amount);
            total_revenue = total_revenue.saturating_add(sub_revenue);

            // Monthly statistics (approximate)
            if subscription.created_ts >= month_start {
                monthly_new_subs = monthly_new_subs.saturating_add(1);
                monthly_revenue = monthly_revenue.saturating_add(subscription.last_amount);
            }

            // Count cancellations (inactive subscriptions created this month)
            if !subscription.active && subscription.created_ts >= month_start {
                monthly_canceled_subs = monthly_canceled_subs.saturating_add(1);
            }
        }

        let average_revenue_per_user = if all_subscriptions.is_empty() {
            0
        } else {
            total_revenue / u64::try_from(all_subscriptions.len()).unwrap_or(1)
        };

        Ok(Overview {
            total_revenue,
            active_subscriptions: active_count,
            inactive_subscriptions: inactive_count,
            total_plans,
            monthly_revenue,
            monthly_new_subscriptions: monthly_new_subs,
            monthly_canceled_subscriptions: monthly_canceled_subs,
            average_revenue_per_user,
            merchant_authority: merchant_data.authority,
            usdc_mint: merchant_data.usdc_mint,
        })
    }

    /// Get all active subscriptions for a merchant with enhanced information
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    ///
    /// # Returns
    /// * `Ok(Vec<DashboardSubscription>)` - List of enhanced subscription data
    ///
    /// # Errors
    /// Returns an error if data fetching fails
    pub fn get_live_subscriptions(&self, merchant: &Pubkey) -> Result<Vec<DashboardSubscription>> {
        let plans = self.client.list_plans(merchant)?;
        let mut dashboard_subscriptions = Vec::new();
        let current_time = Utc::now().timestamp();

        for (plan_address, plan) in plans {
            let subscriptions = self.client.list_subscriptions(&plan_address)?;

            for (sub_address, subscription) in subscriptions {
                let status = DashboardSubscription::calculate_status(&subscription, current_time);
                let days_until_renewal = DashboardSubscription::calculate_days_until_renewal(
                    subscription.next_renewal_ts,
                    current_time,
                );
                let total_paid = u64::from(subscription.renewals) * subscription.last_amount;

                dashboard_subscriptions.push(DashboardSubscription {
                    subscription,
                    address: sub_address,
                    plan: plan.clone(),
                    plan_address,
                    status,
                    days_until_renewal,
                    total_paid,
                });
            }
        }

        Ok(dashboard_subscriptions)
    }

    /// Get analytics for a specific plan
    ///
    /// # Arguments
    /// * `plan` - The plan PDA address
    ///
    /// # Returns
    /// * `Ok(PlanAnalytics)` - Plan analytics data
    ///
    /// # Errors
    /// Returns an error if the plan doesn't exist or data fetching fails
    pub fn get_plan_analytics(&self, plan: &Pubkey) -> Result<PlanAnalytics> {
        // Get plan data
        let plan_data = self
            .client
            .get_plan(plan)?
            .ok_or_else(|| TallyError::AccountNotFound(format!("Plan not found: {plan}")))?;

        // Get all subscriptions for this plan
        let subscriptions = self.client.list_subscriptions(plan)?;

        // Calculate statistics
        let current_time = Utc::now().timestamp();
        let month_start = current_time - (30 * 24 * 60 * 60); // 30 days ago

        let mut active_count = 0;
        let mut inactive_count = 0;
        let mut total_revenue = 0;
        let mut monthly_revenue = 0;
        let mut monthly_new_subs = 0;
        let mut monthly_canceled_subs = 0;
        let mut total_duration_secs = 0i64;
        let mut completed_subscriptions = 0;

        for (_sub_address, subscription) in &subscriptions {
            if subscription.active {
                active_count += 1;
            } else {
                inactive_count += 1;

                // Calculate duration for completed subscriptions
                let duration = current_time - subscription.created_ts;
                total_duration_secs += duration;
                completed_subscriptions += 1;
            }

            // Calculate revenue (renewals * last_amount)
            let sub_revenue = u64::from(subscription.renewals) * subscription.last_amount;
            total_revenue += sub_revenue;

            // Monthly statistics
            if subscription.created_ts >= month_start {
                monthly_new_subs += 1;
                monthly_revenue += subscription.last_amount;
            }

            // Count monthly cancellations
            if !subscription.active && subscription.created_ts >= month_start {
                monthly_canceled_subs += 1;
            }
        }

        let average_duration_days = if completed_subscriptions > 0 {
            (total_duration_secs / i64::from(completed_subscriptions)) as f64 / 86400.0
        } else {
            0.0
        };

        Ok(PlanAnalytics {
            plan: plan_data,
            plan_address: *plan,
            active_count,
            inactive_count,
            total_revenue,
            monthly_revenue,
            monthly_new_subscriptions: monthly_new_subs,
            monthly_canceled_subscriptions: monthly_canceled_subs,
            average_duration_days,
            conversion_rate: None, // Would need additional data to calculate
        })
    }

    /// List all plans for a merchant with basic information
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    ///
    /// # Returns
    /// * `Ok(Vec<(Pubkey, Plan)>)` - List of plan addresses and data
    ///
    /// # Errors
    /// Returns an error if data fetching fails
    pub fn list_merchant_plans(&self, merchant: &Pubkey) -> Result<Vec<(Pubkey, Plan)>> {
        self.client.list_plans(merchant)
    }

    /// Get plan analytics for all plans of a merchant
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    ///
    /// # Returns
    /// * `Ok(Vec<PlanAnalytics>)` - List of analytics for all plans
    ///
    /// # Errors
    /// Returns an error if data fetching fails
    pub fn get_all_plan_analytics(&self, merchant: &Pubkey) -> Result<Vec<PlanAnalytics>> {
        let plans = self.client.list_plans(merchant)?;
        let mut analytics = Vec::new();

        for (plan_address, _plan) in plans {
            let plan_analytics = self.get_plan_analytics(&plan_address)?;
            analytics.push(plan_analytics);
        }

        Ok(analytics)
    }

    // ========================================
    // Event Monitoring Methods
    // ========================================

    /// Subscribe to real-time events for a merchant
    ///
    /// This method sets up event monitoring and returns an `EventStream` that can be
    /// used to track real-time changes to the subscription system.
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address to monitor
    ///
    /// # Returns
    /// * `Ok(EventStream)` - Event stream for real-time monitoring
    ///
    /// # Errors
    /// Returns an error if event monitoring setup fails
    pub fn subscribe_to_events(&self, merchant: &Pubkey) -> Result<EventStream> {
        // For now, return a basic event stream
        // In a full implementation, this would set up WebSocket connections
        // to monitor blockchain events in real-time
        let mut stream = EventStream::new();
        stream.start();

        // Add merchant validation
        if !self.client.account_exists(merchant)? {
            return Err(TallyError::AccountNotFound(format!(
                "Merchant not found: {merchant}"
            )));
        }

        Ok(stream)
    }

    /// Poll for recent events manually
    ///
    /// This method can be used as an alternative to real-time event streaming
    /// for applications that prefer polling-based updates.
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    /// * `since_timestamp` - Only return events after this timestamp
    ///
    /// # Returns
    /// * `Ok(Vec<DashboardEvent>)` - List of recent events
    ///
    /// # Errors
    /// Returns an error if event fetching fails
    pub fn poll_recent_events(
        &self,
        merchant: &Pubkey,
        since_timestamp: i64,
    ) -> Result<Vec<DashboardEvent>> {
        // This is a placeholder implementation
        // In a real implementation, this would query transaction logs
        // and parse program events to reconstruct dashboard events

        let _plans = self.client.list_plans(merchant)?;
        let events = Vec::new(); // Would be populated with actual events

        // Filter events by timestamp
        let filtered_events: Vec<DashboardEvent> = events
            .into_iter()
            .filter(|event: &DashboardEvent| event.timestamp >= since_timestamp)
            .collect();

        Ok(filtered_events)
    }

    /// Get event statistics for a time period
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    /// * `start_timestamp` - Start of time period
    /// * `end_timestamp` - End of time period
    ///
    /// # Returns
    /// * `Ok(HashMap<String, u32>)` - Event type counts
    ///
    /// # Errors
    /// Returns an error if data fetching fails
    pub fn get_event_statistics(
        &self,
        merchant: &Pubkey,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> Result<HashMap<String, u32>> {
        let events = self.poll_recent_events(merchant, start_timestamp)?;
        let mut stats = HashMap::new();

        for event in events {
            if event.timestamp <= end_timestamp {
                let event_type = format!("{:?}", event.event_type);
                *stats.entry(event_type).or_insert(0) += 1;
            }
        }

        Ok(stats)
    }

    // ========================================
    // Utility Methods
    // ========================================

    /// Validate if a merchant exists
    ///
    /// # Arguments
    /// * `merchant` - The merchant PDA address
    ///
    /// # Returns
    /// * `Ok(bool)` - True if merchant exists
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn merchant_exists(&self, merchant: &Pubkey) -> Result<bool> {
        self.client.account_exists(merchant)
    }

    /// Validate if a plan exists
    ///
    /// # Arguments
    /// * `plan` - The plan PDA address
    ///
    /// # Returns
    /// * `Ok(bool)` - True if plan exists
    ///
    /// # Errors
    /// Returns an error if RPC call fails
    pub fn plan_exists(&self, plan: &Pubkey) -> Result<bool> {
        self.client.account_exists(plan)
    }

    /// Get the current timestamp (useful for event filtering)
    #[must_use]
    pub fn current_timestamp() -> i64 {
        Utc::now().timestamp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_types::InitMerchantArgs;
    use solana_sdk::{signature::Keypair, signer::Signer};

    #[test]
    fn test_dashboard_client_creation() {
        let client = DashboardClient::new("http://localhost:8899").unwrap();
        assert_eq!(client.program_id().to_string(), crate::program_id_string());
    }

    #[test]
    fn test_current_timestamp() {
        let timestamp = DashboardClient::current_timestamp();
        assert!(timestamp > 0);

        // Timestamp should be recent (within last minute)
        let now = Utc::now().timestamp();
        assert!((now - timestamp).abs() < 60);
    }

    #[test]
    fn test_event_stream_creation() {
        let client = DashboardClient::new("http://localhost:8899").unwrap();
        let merchant = Keypair::new().pubkey();

        // This will fail because merchant doesn't exist, but tests the error path
        let result = client.subscribe_to_events(&merchant);
        assert!(result.is_err());
    }

    #[test]
    fn test_merchant_args_validation() {
        let client = DashboardClient::new("http://localhost:8899").unwrap();
        let authority = Keypair::new();

        // Test invalid platform fee (over 1000 basis points)
        let invalid_args = InitMerchantArgs {
            usdc_mint: Keypair::new().pubkey(),
            treasury_ata: Keypair::new().pubkey(),
            platform_fee_bps: 1001, // Invalid: over 10%
        };

        let result = client.provision_merchant(&authority, &invalid_args);
        assert!(result.is_err());
    }

    #[test]
    fn test_overview_calculation_methods() {
        use crate::dashboard_types::Overview;

        let overview = Overview {
            total_revenue: 1_000_000_000, // 1,000 USDC
            active_subscriptions: 80,
            inactive_subscriptions: 20,
            total_plans: 5,
            monthly_revenue: 100_000_000, // 100 USDC
            monthly_new_subscriptions: 10,
            monthly_canceled_subscriptions: 5,
            average_revenue_per_user: 10_000_000, // 10 USDC
            merchant_authority: Keypair::new().pubkey(),
            usdc_mint: Keypair::new().pubkey(),
        };

        // Use epsilon comparison for float values
        assert!((overview.total_revenue_formatted() - 1000.0).abs() < f64::EPSILON);
        assert!((overview.monthly_revenue_formatted() - 100.0).abs() < f64::EPSILON);
        assert!((overview.average_revenue_per_user_formatted() - 10.0).abs() < f64::EPSILON);
        assert!((overview.churn_rate() - 20.0).abs() < f64::EPSILON); // 20 out of 100 = 20%
    }

    #[test]
    fn test_dashboard_event_functionality() {
        use crate::dashboard_types::{DashboardEvent, DashboardEventType};
        use std::collections::HashMap;

        let mut metadata = HashMap::new();
        metadata.insert("plan_name".to_string(), "Premium Plan".to_string());

        let event = DashboardEvent {
            event_type: DashboardEventType::SubscriptionStarted,
            plan_address: Some(Keypair::new().pubkey()),
            subscription_address: Some(Keypair::new().pubkey()),
            subscriber: Some(Keypair::new().pubkey()),
            amount: Some(5_000_000), // 5 USDC
            transaction_signature: Some("test_sig_123".to_string()),
            timestamp: chrono::Utc::now().timestamp(),
            metadata,
        };

        assert_eq!(event.amount_formatted(), Some(5.0));
        assert!(event.affects_revenue());
        assert!(event.affects_subscription_count());

        // Test different event types
        let payment_failed_event = DashboardEvent {
            event_type: DashboardEventType::PaymentFailed,
            plan_address: None,
            subscription_address: None,
            subscriber: None,
            amount: None,
            transaction_signature: None,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: HashMap::new(),
        };

        assert!(!payment_failed_event.affects_revenue());
        assert!(!payment_failed_event.affects_subscription_count());
    }

    #[test]
    fn test_event_stream_buffer_management() {
        use crate::dashboard_types::{DashboardEvent, DashboardEventType, EventStream};
        use std::collections::HashMap;

        let mut stream = EventStream::with_buffer_size(2);
        assert!(!stream.is_active);

        stream.start();
        assert!(stream.is_active);

        // Add events to test buffer overflow
        let event1 = DashboardEvent {
            event_type: DashboardEventType::SubscriptionStarted,
            plan_address: None,
            subscription_address: None,
            subscriber: None,
            amount: None,
            transaction_signature: None,
            timestamp: chrono::Utc::now().timestamp() - 3600,
            metadata: HashMap::new(),
        };

        let event2 = DashboardEvent {
            event_type: DashboardEventType::SubscriptionRenewed,
            plan_address: None,
            subscription_address: None,
            subscriber: None,
            amount: None,
            transaction_signature: None,
            timestamp: chrono::Utc::now().timestamp() - 1800,
            metadata: HashMap::new(),
        };

        let event3 = DashboardEvent {
            event_type: DashboardEventType::PaymentFailed,
            plan_address: None,
            subscription_address: None,
            subscriber: None,
            amount: None,
            transaction_signature: None,
            timestamp: chrono::Utc::now().timestamp(),
            metadata: HashMap::new(),
        };

        stream.add_event(event1);
        assert_eq!(stream.events.len(), 1);

        stream.add_event(event2);
        assert_eq!(stream.events.len(), 2);

        // This should remove the first event due to buffer limit
        stream.add_event(event3);
        assert_eq!(stream.events.len(), 2);

        // Test event type filtering
        let failed_events = stream.events_of_type(&DashboardEventType::PaymentFailed);
        assert_eq!(failed_events.len(), 1);

        stream.stop();
        assert!(!stream.is_active);
    }
}

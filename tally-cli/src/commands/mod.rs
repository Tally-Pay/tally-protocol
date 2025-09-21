//! Command implementations for the Tally CLI
//!
//! This module contains the individual command implementations, each in their own file
//! for better organization and maintainability.

pub mod create_plan;
pub mod dashboard;
pub mod deactivate_plan;
pub mod init_config;
pub mod init_merchant;
pub mod list_plans;
pub mod list_subs;
pub mod simulate_events;
pub mod withdraw_fees;

// Re-export command execution functions for easy access
pub use create_plan::execute as execute_create_plan;
pub use dashboard::execute as execute_dashboard_command;
pub use deactivate_plan::execute as execute_deactivate_plan;
pub use init_config::execute as execute_init_config;
pub use init_merchant::execute as execute_init_merchant;
pub use list_plans::execute as execute_list_plans;
pub use list_subs::execute as execute_list_subs;
pub use simulate_events::execute as execute_simulate_events;
pub use withdraw_fees::execute as execute_withdraw_fees;

//! Deactivate plan command implementation

use anyhow::{anyhow, Result};
use std::str::FromStr;
use tally_sdk::{
    load_keypair, pda,
    SimpleTallyClient,
};
use anchor_lang::prelude::Pubkey;
use anchor_client::solana_sdk::signature::Signer;
use tracing::info;

/// Execute the deactivate plan command
///
/// # Errors
/// Returns error if plan deactivation fails due to invalid parameters, network issues, or Solana program errors
#[allow(clippy::cognitive_complexity)] // Complex validation logic for plan deactivation
pub async fn execute(
    tally_client: &SimpleTallyClient,
    plan_str: &str,
    authority_path: Option<&str>,
) -> Result<String> {
    info!("Starting plan deactivation");

    // Parse plan PDA address
    let plan_pda = Pubkey::from_str(plan_str)
        .map_err(|e| anyhow!("Invalid plan PDA address '{plan_str}': {e}"))?;
    info!("Using plan PDA: {}", plan_pda);

    // Load authority keypair
    let authority = load_keypair(authority_path)?;
    info!("Using authority: {}", authority.pubkey());

    // Fetch and validate plan account using tally-sdk
    let plan = tally_client
        .get_plan(&plan_pda)?
        .ok_or_else(|| anyhow!("Plan account does not exist at address: {plan_pda}"))?;

    // Validate authority matches merchant authority by computing expected merchant PDA
    let authority_pubkey = Pubkey::from(authority.pubkey().to_bytes());
    let expected_merchant_pda = pda::merchant_address(&authority_pubkey)?;
    if plan.merchant != expected_merchant_pda {
        return Err(anyhow!(
            "Authority mismatch: this authority ({}) does not own the merchant ({}) for this plan. Expected merchant: {}",
            authority.pubkey(),
            plan.merchant,
            expected_merchant_pda
        ));
    }

    // Check if plan is already deactivated
    if !plan.active {
        return Err(anyhow!(
            "Plan '{}' is already deactivated",
            plan.plan_id_str()
        ));
    }

    info!(
        "Plan '{}' is currently active, proceeding with deactivation",
        plan.plan_id_str()
    );

    // IMPORTANT NOTE: The current Anchor program does not have a deactivate_plan instruction.
    // This implementation assumes such an instruction would be added to the program.
    // For now, we'll return an error explaining this limitation.

    Err(anyhow!(
        "PROGRAM LIMITATION: The current Tally subscription program does not have a 'deactivate_plan' instruction.\n\
        To implement plan deactivation, the following would be needed:\n\
        1. Add a 'deactivate_plan' instruction to the Anchor program that sets plan.active = false\n\
        2. The instruction would have discriminator: [91, 38, 214, 232, 172, 21, 30, 93] (computed from SHA256('global:deactivate_plan'))\n\
        3. Required accounts: payer (authority), authority (signer), plan (PDA, mutable)\n\
        4. No additional instruction data needed besides the discriminator\n\
        \n\
        Current plan details:\n\
        - Plan ID: {}\n\
        - Plan Name: {}\n\
        - Currently Active: {}\n\
        - Merchant: {}",
        plan.plan_id_str(),
        plan.name_str(),
        plan.active,
        plan.merchant
    ))

    // This would be the implementation if the instruction existed:
    /*
    // Build deactivate_plan instruction manually (since no builder exists in tally-sdk)
    let instruction = build_deactivate_plan_instruction(
        &authority.pubkey(),
        &plan_pda,
        tally_client.program_id(),
    )?;

    info!("Building transaction with deactivate_plan instruction");

    // Get recent blockhash
    let recent_blockhash = tally_client
        .rpc()
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())?
        .0;

    // Create and sign transaction
    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&authority.pubkey()));
    transaction.sign(&[&authority], recent_blockhash);

    info!("Submitting transaction...");

    // Submit transaction
    let signature = tally_client
        .rpc()
        .send_and_confirm_transaction_with_spinner(&transaction)?;

    info!("Transaction confirmed: {}", signature);

    // Return success message
    Ok(format!(
        "Plan deactivated successfully!\nPlan PDA: {}\nPlan ID: {}\nTransaction signature: {}",
        plan_pda,
        plan.plan_id_str(),
        signature
    ))
    */
}

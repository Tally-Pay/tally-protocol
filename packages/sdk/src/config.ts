// Solana system program ID (this is a constant and should never change)
const SYSTEM_PROGRAM_ID_CONSTANT = '11111111111111111111111111111111';

/**
 * Get the Tally program ID from environment variable or use provided override.
 *
 * IMPORTANT: Requires TALLY_PROGRAM_ID environment variable to be set.
 * Throws an error if not provided to prevent using wrong program ID.
 *
 * @param override Optional program ID override (for testing)
 * @returns The program ID to use
 * @throws Error if TALLY_PROGRAM_ID is not set and no override provided
 *
 * @example
 * ```bash
 * export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111
 * ```
 */
export function getProgramId(override?: string): string {
  if (override) return override;

  const programId = process.env.TALLY_PROGRAM_ID;
  if (!programId) {
    throw new Error(
      'TALLY_PROGRAM_ID environment variable is required. ' +
      'Set it to your deployed program ID (localnet/devnet/mainnet).\n' +
      'Example: export TALLY_PROGRAM_ID=YourProgramIdHere111111111111111111111111111'
    );
  }

  return programId;
}

/**
 * Get the Solana system program ID.
 *
 * This is always 11111111111111111111111111111111 (Solana's system program).
 *
 * @param override Optional system program ID override (for testing)
 * @returns The system program ID
 */
export function getSystemProgramId(override?: string): string {
  return override || SYSTEM_PROGRAM_ID_CONSTANT;
}

// Export for backward compatibility
// NOTE: Will throw if TALLY_PROGRAM_ID is not set
export const PROGRAM_ID = getProgramId();
export const SYSTEM_PROGRAM_ID = getSystemProgramId();
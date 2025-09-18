// Default fallback - should be overridden in production
const DEFAULT_PROGRAM_ID = '11111111111111111111111111111111';
const DEFAULT_SYSTEM_PROGRAM_ID = '11111111111111111111111111111111';

/**
 * Get the program ID from environment variable or use provided override
 * @param override Optional program ID override
 * @returns The program ID to use
 */
export function getProgramId(override?: string): string {
  return override || process.env.PROGRAM_ID || DEFAULT_PROGRAM_ID;
}

/**
 * Get the system program ID from environment variable or use default
 * @param override Optional system program ID override
 * @returns The system program ID to use
 */
export function getSystemProgramId(override?: string): string {
  return override || process.env.SYSTEM_PROGRAM_ID || DEFAULT_SYSTEM_PROGRAM_ID;
}

// Export for backward compatibility - uses environment variable if available
export const PROGRAM_ID = getProgramId();
export const SYSTEM_PROGRAM_ID = getSystemProgramId();
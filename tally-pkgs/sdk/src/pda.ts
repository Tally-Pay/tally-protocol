import { PublicKey } from '@solana/web3.js';
import { getProgramId } from './config';

/**
 * Derive merchant PDA
 * @param authority The merchant authority public key
 * @param programId Optional program ID override
 * @returns The merchant PDA and bump seed
 */
export function deriveMerchantPda(
  authority: PublicKey,
  programId?: string
): [PublicKey, number] {
  const programPublicKey = new PublicKey(getProgramId(programId));
  return PublicKey.findProgramAddressSync(
    [Buffer.from('merchant'), authority.toBuffer()],
    programPublicKey
  );
}

/**
 * Derive plan PDA
 * @param merchant The merchant PDA
 * @param planId The plan ID string
 * @param programId Optional program ID override
 * @returns The plan PDA and bump seed
 */
export function derivePlanPda(
  merchant: PublicKey,
  planId: string,
  programId?: string
): [PublicKey, number] {
  const programPublicKey = new PublicKey(getProgramId(programId));
  return PublicKey.findProgramAddressSync(
    [Buffer.from('plan'), merchant.toBuffer(), Buffer.from(planId)],
    programPublicKey
  );
}

/**
 * Derive subscription PDA
 * @param plan The plan PDA
 * @param subscriber The subscriber public key
 * @param programId Optional program ID override
 * @returns The subscription PDA and bump seed
 */
export function deriveSubscriptionPda(
  plan: PublicKey,
  subscriber: PublicKey,
  programId?: string
): [PublicKey, number] {
  const programPublicKey = new PublicKey(getProgramId(programId));
  return PublicKey.findProgramAddressSync(
    [Buffer.from('subscription'), plan.toBuffer(), subscriber.toBuffer()],
    programPublicKey
  );
}
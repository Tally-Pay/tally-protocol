// Types
export interface Merchant {
  authority: string;
  feeBps: number;
}

export interface Plan {
  merchant: string;
  planId: string;
  price: bigint;
  periodSeconds: number;
}

export interface Subscription {
  plan: string;
  subscriber: string;
  nextRenewalAt: number;
}

// Re-export configuration utilities
export { getProgramId, getSystemProgramId, PROGRAM_ID, SYSTEM_PROGRAM_ID } from './config';

// Import for internal use
import { getProgramId as _getProgramId } from './config';

export interface SubscriptionClientConfig {
  rpcUrl: string;
  programId?: string;
}

export class SubscriptionClient {
  private readonly programId: string;

  constructor(private readonly config: SubscriptionClientConfig) {
    this.programId = _getProgramId(config.programId);
  }

  /**
   * Get the program ID used by this client instance
   */
  getProgramId(): string {
    return this.programId;
  }

  async fetchMerchant(_merchant: string): Promise<Merchant | null> {
    void this.config;
    // TODO: Implement actual RPC calls using this.programId
    return null;
  }

  async fetchPlan(_merchant: string, _planId: string): Promise<Plan | null> {
    // TODO: Implement actual RPC calls using this.programId
    return null;
  }
}

// Re-export IDL utilities
export { getIdl, rawIdl, type IdlConfig } from './idl';

// Re-export PDA utilities
export { deriveMerchantPda, derivePlanPda, deriveSubscriptionPda } from './pda';

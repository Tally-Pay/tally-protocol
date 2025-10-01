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

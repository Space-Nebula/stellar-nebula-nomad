export interface WebhookRegistration {
  id: string;
  url: string;
  events: string[];
  secret: string;
  active: boolean;
  createdAt: Date;
  filters?: EventFilter[];
}

export interface EventFilter {
  field: string;
  operator: "eq" | "neq" | "gt" | "lt" | "contains";
  value: any;
}

export interface ContractEvent {
  type: string;
  contractId: string;
  ledger: number;
  txHash: string;
  timestamp: Date;
  data: Record<string, any>;
}

export interface WebhookPayload {
  event: ContractEvent;
  webhookId: string;
  deliveryId: string;
  timestamp: Date;
  signature: string;
}

export interface DeliveryAttempt {
  id: string;
  webhookId: string;
  eventId: string;
  attempt: number;
  status: "pending" | "success" | "failed";
  statusCode?: number;
  response?: string;
  error?: string;
  timestamp: Date;
}

export interface WebhookConfig {
  maxRetries: number;
  retryDelays: number[]; // milliseconds for each retry
  timeout: number;
  batchSize: number;
}

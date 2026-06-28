import crypto from "crypto";
import axios, { AxiosError } from "axios";
import {
  WebhookRegistration,
  ContractEvent,
  WebhookPayload,
  DeliveryAttempt,
  WebhookConfig,
  EventFilter,
} from "./types";
import { Database } from "./database";

export class WebhookManager {
  private db: Database;
  private config: WebhookConfig;
  private deliveryQueue: Map<string, NodeJS.Timeout>;

  constructor(db: Database, config?: Partial<WebhookConfig>) {
    this.db = db;
    this.config = {
      maxRetries: config?.maxRetries || 3,
      retryDelays: config?.retryDelays || [1000, 5000, 15000],
      timeout: config?.timeout || 10000,
      batchSize: config?.batchSize || 100,
    };
    this.deliveryQueue = new Map();
  }

  /**
   * Register a new webhook
   */
  async registerWebhook(
    url: string,
    events: string[],
    filters?: EventFilter[],
  ): Promise<WebhookRegistration> {
    const webhook: WebhookRegistration = {
      id: this.generateId(),
      url,
      events,
      secret: this.generateSecret(),
      active: true,
      createdAt: new Date(),
      filters,
    };

    await this.db.saveWebhook(webhook);
    return webhook;
  }

  /**
   * Update webhook configuration
   */
  async updateWebhook(
    id: string,
    updates: Partial<WebhookRegistration>,
  ): Promise<void> {
    await this.db.updateWebhook(id, updates);
  }

  /**
   * Delete a webhook
   */
  async deleteWebhook(id: string): Promise<void> {
    await this.db.deleteWebhook(id);

    // Cancel any pending deliveries
    if (this.deliveryQueue.has(id)) {
      clearTimeout(this.deliveryQueue.get(id)!);
      this.deliveryQueue.delete(id);
    }
  }

  /**
   * Process incoming contract event
   */
  async processEvent(event: ContractEvent): Promise<void> {
    const webhooks = await this.db.getActiveWebhooks();

    for (const webhook of webhooks) {
      if (this.shouldDeliverEvent(webhook, event)) {
        await this.queueDelivery(webhook, event);
      }
    }
  }

  /**
   * Check if event should be delivered to webhook
   */
  private shouldDeliverEvent(
    webhook: WebhookRegistration,
    event: ContractEvent,
  ): boolean {
    // Check if event type matches
    if (!webhook.events.includes(event.type) && !webhook.events.includes("*")) {
      return false;
    }

    // Apply filters
    if (webhook.filters && webhook.filters.length > 0) {
      return this.matchesFilters(event, webhook.filters);
    }

    return true;
  }

  /**
   * Check if event matches filters
   */
  private matchesFilters(
    event: ContractEvent,
    filters: EventFilter[],
  ): boolean {
    return filters.every((filter) => {
      const value = this.getNestedValue(event.data, filter.field);

      switch (filter.operator) {
        case "eq":
          return value === filter.value;
        case "neq":
          return value !== filter.value;
        case "gt":
          return value > filter.value;
        case "lt":
          return value < filter.value;
        case "contains":
          return String(value).includes(String(filter.value));
        default:
          return false;
      }
    });
  }

  /**
   * Queue event delivery
   */
  private async queueDelivery(
    webhook: WebhookRegistration,
    event: ContractEvent,
  ): Promise<void> {
    const deliveryId = this.generateId();
    const payload = this.createPayload(webhook, event, deliveryId);

    await this.deliverWebhook(webhook, payload, 0);
  }

  /**
   * Deliver webhook with retry logic
   */
  private async deliverWebhook(
    webhook: WebhookRegistration,
    payload: WebhookPayload,
    attemptNumber: number,
  ): Promise<void> {
    const attempt: DeliveryAttempt = {
      id: this.generateId(),
      webhookId: webhook.id,
      eventId: payload.event.txHash,
      attempt: attemptNumber,
      status: "pending",
      timestamp: new Date(),
    };

    try {
      const response = await axios.post(webhook.url, payload, {
        timeout: this.config.timeout,
        headers: {
          "Content-Type": "application/json",
          "X-Webhook-Signature": payload.signature,
          "X-Webhook-ID": webhook.id,
          "X-Delivery-ID": payload.deliveryId,
        },
      });

      attempt.status = "success";
      attempt.statusCode = response.status;
      attempt.response = JSON.stringify(response.data);

      await this.db.saveDeliveryAttempt(attempt);
    } catch (error) {
      attempt.status = "failed";

      if (error instanceof AxiosError) {
        attempt.statusCode = error.response?.status;
        attempt.error = error.message;
      } else {
        attempt.error = String(error);
      }

      await this.db.saveDeliveryAttempt(attempt);

      // Retry logic
      if (attemptNumber < this.config.maxRetries) {
        const delay = this.config.retryDelays[attemptNumber] || 30000;

        const timeoutId = setTimeout(() => {
          this.deliverWebhook(webhook, payload, attemptNumber + 1);
          this.deliveryQueue.delete(webhook.id);
        }, delay);

        this.deliveryQueue.set(webhook.id, timeoutId);
      } else {
        // Max retries reached, mark webhook as inactive if too many failures
        await this.handleMaxRetriesReached(webhook);
      }
    }
  }

  /**
   * Handle max retries reached
   */
  private async handleMaxRetriesReached(
    webhook: WebhookRegistration,
  ): Promise<void> {
    const recentFailures = await this.db.getRecentFailures(webhook.id, 10);

    // If last 10 deliveries failed, deactivate webhook
    if (recentFailures.length >= 10) {
      await this.updateWebhook(webhook.id, { active: false });
    }
  }

  /**
   * Create webhook payload
   */
  private createPayload(
    webhook: WebhookRegistration,
    event: ContractEvent,
    deliveryId: string,
  ): WebhookPayload {
    const payload: WebhookPayload = {
      event,
      webhookId: webhook.id,
      deliveryId,
      timestamp: new Date(),
      signature: "",
    };

    payload.signature = this.generateSignature(payload, webhook.secret);
    return payload;
  }

  /**
   * Generate HMAC signature
   */
  private generateSignature(payload: WebhookPayload, secret: string): string {
    const data = JSON.stringify({
      event: payload.event,
      webhookId: payload.webhookId,
      deliveryId: payload.deliveryId,
      timestamp: payload.timestamp,
    });

    return crypto.createHmac("sha256", secret).update(data).digest("hex");
  }

  /**
   * Get nested value from object
   */
  private getNestedValue(obj: any, path: string): any {
    return path.split(".").reduce((current, key) => current?.[key], obj);
  }

  /**
   * Generate unique ID
   */
  private generateId(): string {
    return `${Date.now()}-${crypto.randomBytes(8).toString("hex")}`;
  }

  /**
   * Generate webhook secret
   */
  private generateSecret(): string {
    return crypto.randomBytes(32).toString("hex");
  }

  /**
   * Get webhook statistics
   */
  async getWebhookStats(webhookId: string): Promise<{
    totalDeliveries: number;
    successfulDeliveries: number;
    failedDeliveries: number;
    averageResponseTime: number;
  }> {
    return this.db.getWebhookStats(webhookId);
  }
}

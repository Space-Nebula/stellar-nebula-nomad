import { Pool } from "pg";
import { WebhookRegistration, DeliveryAttempt } from "./types";

export class Database {
  private pool: Pool;

  constructor(connectionString: string) {
    this.pool = new Pool({ connectionString });
    this.initSchema();
  }

  private async initSchema(): Promise<void> {
    await this.pool.query(`
      CREATE TABLE IF NOT EXISTS webhooks (
        id VARCHAR(255) PRIMARY KEY,
        url TEXT NOT NULL,
        events JSONB NOT NULL,
        secret VARCHAR(255) NOT NULL,
        active BOOLEAN DEFAULT true,
        filters JSONB,
        created_at TIMESTAMP DEFAULT NOW()
      );

      CREATE TABLE IF NOT EXISTS delivery_attempts (
        id VARCHAR(255) PRIMARY KEY,
        webhook_id VARCHAR(255) REFERENCES webhooks(id) ON DELETE CASCADE,
        event_id VARCHAR(255) NOT NULL,
        attempt INTEGER NOT NULL,
        status VARCHAR(50) NOT NULL,
        status_code INTEGER,
        response TEXT,
        error TEXT,
        timestamp TIMESTAMP DEFAULT NOW()
      );

      CREATE INDEX IF NOT EXISTS idx_webhook_active ON webhooks(active);
      CREATE INDEX IF NOT EXISTS idx_delivery_webhook ON delivery_attempts(webhook_id);
      CREATE INDEX IF NOT EXISTS idx_delivery_status ON delivery_attempts(status);
    `);
  }

  async saveWebhook(webhook: WebhookRegistration): Promise<void> {
    await this.pool.query(
      `INSERT INTO webhooks (id, url, events, secret, active, filters, created_at)
       VALUES ($1, $2, $3, $4, $5, $6, $7)`,
      [
        webhook.id,
        webhook.url,
        JSON.stringify(webhook.events),
        webhook.secret,
        webhook.active,
        JSON.stringify(webhook.filters || []),
        webhook.createdAt,
      ],
    );
  }

  async updateWebhook(
    id: string,
    updates: Partial<WebhookRegistration>,
  ): Promise<void> {
    const fields: string[] = [];
    const values: any[] = [];
    let paramCount = 1;

    if (updates.url !== undefined) {
      fields.push(`url = $${paramCount++}`);
      values.push(updates.url);
    }
    if (updates.events !== undefined) {
      fields.push(`events = $${paramCount++}`);
      values.push(JSON.stringify(updates.events));
    }
    if (updates.active !== undefined) {
      fields.push(`active = $${paramCount++}`);
      values.push(updates.active);
    }
    if (updates.filters !== undefined) {
      fields.push(`filters = $${paramCount++}`);
      values.push(JSON.stringify(updates.filters));
    }

    if (fields.length > 0) {
      values.push(id);
      await this.pool.query(
        `UPDATE webhooks SET ${fields.join(", ")} WHERE id = $${paramCount}`,
        values,
      );
    }
  }

  async deleteWebhook(id: string): Promise<void> {
    await this.pool.query("DELETE FROM webhooks WHERE id = $1", [id]);
  }

  async getActiveWebhooks(): Promise<WebhookRegistration[]> {
    const result = await this.pool.query(
      "SELECT * FROM webhooks WHERE active = true",
    );

    return result.rows.map((row) => ({
      id: row.id,
      url: row.url,
      events: row.events,
      secret: row.secret,
      active: row.active,
      createdAt: row.created_at,
      filters: row.filters,
    }));
  }

  async saveDeliveryAttempt(attempt: DeliveryAttempt): Promise<void> {
    await this.pool.query(
      `INSERT INTO delivery_attempts 
       (id, webhook_id, event_id, attempt, status, status_code, response, error, timestamp)
       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)`,
      [
        attempt.id,
        attempt.webhookId,
        attempt.eventId,
        attempt.attempt,
        attempt.status,
        attempt.statusCode,
        attempt.response,
        attempt.error,
        attempt.timestamp,
      ],
    );
  }

  async getRecentFailures(
    webhookId: string,
    limit: number,
  ): Promise<DeliveryAttempt[]> {
    const result = await this.pool.query(
      `SELECT * FROM delivery_attempts 
       WHERE webhook_id = $1 AND status = 'failed'
       ORDER BY timestamp DESC
       LIMIT $2`,
      [webhookId, limit],
    );

    return result.rows;
  }

  async getWebhookStats(webhookId: string): Promise<any> {
    const result = await this.pool.query(
      `SELECT 
        COUNT(*) as total_deliveries,
        COUNT(CASE WHEN status = 'success' THEN 1 END) as successful_deliveries,
        COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed_deliveries,
        AVG(CASE WHEN status_code IS NOT NULL THEN 1 ELSE 0 END) as avg_response_time
       FROM delivery_attempts
       WHERE webhook_id = $1`,
      [webhookId],
    );

    return (
      result.rows[0] || {
        total_deliveries: 0,
        successful_deliveries: 0,
        failed_deliveries: 0,
        avg_response_time: 0,
      }
    );
  }

  async close(): Promise<void> {
    await this.pool.end();
  }
}

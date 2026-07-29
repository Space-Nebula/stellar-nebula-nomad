import { Pool, QueryResult } from "pg";
import {
  DeviceRegistration,
  NotificationPreference,
  StoredDeviceRegistration,
  StoredPreference,
  NotificationEventType,
  NotificationChannel,
} from "./types";

export class NotificationDatabase {
  private pool: Pool;

  constructor(databaseUrl: string) {
    this.pool = new Pool({ connectionString: databaseUrl });
  }

  async init(): Promise<void> {
    await this.pool.query(`
      CREATE TABLE IF NOT EXISTS device_registrations (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        user_id TEXT NOT NULL,
        device_token TEXT NOT NULL,
        platform TEXT NOT NULL CHECK (platform IN ('ios', 'android', 'web')),
        active BOOLEAN DEFAULT true,
        created_at TIMESTAMPTZ DEFAULT NOW(),
        updated_at TIMESTAMPTZ DEFAULT NOW()
      );

      CREATE INDEX IF NOT EXISTS idx_device_registrations_user
        ON device_registrations(user_id);

      CREATE UNIQUE INDEX IF NOT EXISTS idx_device_registrations_token
        ON device_registrations(device_token);

      CREATE TABLE IF NOT EXISTS notification_preferences (
        id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
        user_id TEXT NOT NULL,
        channel TEXT NOT NULL CHECK (channel IN ('push', 'in_app', 'email')),
        event_type TEXT NOT NULL,
        enabled BOOLEAN DEFAULT true,
        updated_at TIMESTAMPTZ DEFAULT NOW(),
        UNIQUE(user_id, channel, event_type)
      );

      CREATE INDEX IF NOT EXISTS idx_notification_preferences_user
        ON notification_preferences(user_id);
    `);
  }

  async registerDevice(
    userId: string,
    deviceToken: string,
    platform: "ios" | "android" | "web",
  ): Promise<void> {
    await this.pool.query(
      `INSERT INTO device_registrations (user_id, device_token, platform)
       VALUES ($1, $2, $3)
       ON CONFLICT (device_token)
       DO UPDATE SET user_id = EXCLUDED.user_id, active = true, updated_at = NOW()`,
      [userId, deviceToken, platform],
    );
  }

  async unregisterDevice(deviceToken: string): Promise<void> {
    await this.pool.query(
      `UPDATE device_registrations SET active = false, updated_at = NOW()
       WHERE device_token = $1`,
      [deviceToken],
    );
  }

  async getActiveDevices(userId: string): Promise<DeviceRegistration[]> {
    const result: QueryResult<StoredDeviceRegistration> = await this.pool.query(
      `SELECT * FROM device_registrations
       WHERE user_id = $1 AND active = true`,
      [userId],
    );
    return result.rows.map(this.mapDeviceRow);
  }

  async getAllActiveDevices(): Promise<DeviceRegistration[]> {
    const result: QueryResult<StoredDeviceRegistration> = await this.pool.query(
      `SELECT * FROM device_registrations WHERE active = true`,
    );
    return result.rows.map(this.mapDeviceRow);
  }

  async setPreference(
    userId: string,
    channel: NotificationChannel,
    eventType: NotificationEventType,
    enabled: boolean,
  ): Promise<void> {
    await this.pool.query(
      `INSERT INTO notification_preferences (user_id, channel, event_type, enabled)
       VALUES ($1, $2, $3, $4)
       ON CONFLICT (user_id, channel, event_type)
       DO UPDATE SET enabled = EXCLUDED.enabled, updated_at = NOW()`,
      [userId, channel, eventType, enabled],
    );
  }

  async getPreferences(
    userId: string,
  ): Promise<NotificationPreference[]> {
    const result: QueryResult<StoredPreference> = await this.pool.query(
      `SELECT * FROM notification_preferences WHERE user_id = $1`,
      [userId],
    );
    return result.rows.map(this.mapPreferenceRow);
  }

  async getDefaultPreferences(
    userId: string,
  ): Promise<NotificationPreference[]> {
    const allEventTypes: NotificationEventType[] = [
      "low_resources",
      "rare_discovery",
      "crafting_complete",
      "yield_claimed",
      "ship_minted",
      "nebula_scanned",
      "harvest_complete",
      "stake_matured",
      "account_alert",
    ];
    const channels: NotificationChannel[] = ["push", "in_app"];
    const prefs: NotificationPreference[] = [];
    for (const channel of channels) {
      for (const eventType of allEventTypes) {
        prefs.push({
          userId,
          channel,
          eventType,
          enabled: true,
          updatedAt: new Date(),
        });
      }
    }
    return prefs;
  }

  async isEventEnabled(
    userId: string,
    channel: NotificationChannel,
    eventType: NotificationEventType,
  ): Promise<boolean> {
    const result = await this.pool.query(
      `SELECT enabled FROM notification_preferences
       WHERE user_id = $1 AND channel = $2 AND event_type = $3`,
      [userId, channel, eventType],
    );
    if (result.rows.length === 0) return true;
    return result.rows[0].enabled;
  }

  private mapDeviceRow(row: StoredDeviceRegistration): DeviceRegistration {
    return {
      userId: row.user_id,
      deviceToken: row.device_token,
      platform: row.platform,
      active: row.active,
      createdAt: row.created_at,
      updatedAt: row.updated_at,
    };
  }

  private mapPreferenceRow(row: StoredPreference): NotificationPreference {
    return {
      userId: row.user_id,
      channel: row.channel as NotificationChannel,
      eventType: row.event_type as NotificationEventType,
      enabled: row.enabled,
      updatedAt: row.updated_at,
    };
  }
}

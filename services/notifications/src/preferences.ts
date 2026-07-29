import {
  NotificationPreference,
  NotificationChannel,
  NotificationEventType,
} from "./types";
import { NotificationDatabase } from "./database";

export class PreferenceManager {
  private db: NotificationDatabase;

  constructor(db: NotificationDatabase) {
    this.db = db;
  }

  async setPreference(
    userId: string,
    channel: NotificationChannel,
    eventType: NotificationEventType,
    enabled: boolean,
  ): Promise<void> {
    await this.db.setPreference(userId, channel, eventType, enabled);
  }

  async setBulkPreferences(
    userId: string,
    preferences: Array<{
      channel: NotificationChannel;
      eventType: NotificationEventType;
      enabled: boolean;
    }>,
  ): Promise<void> {
    for (const pref of preferences) {
      await this.db.setPreference(
        userId,
        pref.channel,
        pref.eventType,
        pref.enabled,
      );
    }
  }

  async getPreferences(userId: string): Promise<NotificationPreference[]> {
    const stored = await this.db.getPreferences(userId);
    if (stored.length === 0) {
      return this.db.getDefaultPreferences(userId);
    }
    return stored;
  }

  async isEventEnabled(
    userId: string,
    channel: NotificationChannel,
    eventType: NotificationEventType,
  ): Promise<boolean> {
    return this.db.isEventEnabled(userId, channel, eventType);
  }

  async enableAll(userId: string): Promise<void> {
    const defaults = await this.db.getDefaultPreferences(userId);
    for (const pref of defaults) {
      await this.db.setPreference(
        userId,
        pref.channel,
        pref.eventType,
        true,
      );
    }
  }

  async disableAll(userId: string): Promise<void> {
    const defaults = await this.db.getDefaultPreferences(userId);
    for (const pref of defaults) {
      await this.db.setPreference(
        userId,
        pref.channel,
        pref.eventType,
        false,
      );
    }
  }

  async disableChannel(
    userId: string,
    channel: NotificationChannel,
  ): Promise<void> {
    const prefs = await this.db.getPreferences(userId);
    const channelPrefs = prefs.filter((p) => p.channel === channel);
    for (const pref of channelPrefs) {
      await this.db.setPreference(
        userId,
        pref.channel,
        pref.eventType,
        false,
      );
    }
  }
}

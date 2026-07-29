export type NotificationChannel = "push" | "in_app" | "email";
export type NotificationEventType =
  | "low_resources"
  | "rare_discovery"
  | "crafting_complete"
  | "yield_claimed"
  | "ship_minted"
  | "nebula_scanned"
  | "harvest_complete"
  | "stake_matured"
  | "account_alert"
  | "all";

export interface DeviceRegistration {
  userId: string;
  deviceToken: string;
  platform: "ios" | "android" | "web";
  active: boolean;
  createdAt: Date;
  updatedAt: Date;
}

export interface NotificationPreference {
  userId: string;
  channel: NotificationChannel;
  eventType: NotificationEventType;
  enabled: boolean;
  updatedAt: Date;
}

export interface PushNotificationPayload {
  title: string;
  body: string;
  data?: Record<string, string>;
  imageUrl?: string;
  badge?: number;
  sound?: string;
}

export interface StoredDeviceRegistration {
  id: string;
  user_id: string;
  device_token: string;
  platform: "ios" | "android" | "web";
  active: boolean;
  created_at: Date;
  updated_at: Date;
}

export interface StoredPreference {
  id: string;
  user_id: string;
  channel: string;
  event_type: string;
  enabled: boolean;
  updated_at: Date;
}

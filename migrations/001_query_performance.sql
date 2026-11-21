CREATE INDEX IF NOT EXISTS idx_device_registrations_active_user
  ON device_registrations (active, user_id);

CREATE INDEX IF NOT EXISTS idx_notification_preferences_lookup
  ON notification_preferences (user_id, channel, event_type);

CREATE INDEX IF NOT EXISTS idx_webhook_active_created
  ON webhooks (active, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_delivery_webhook_status_timestamp
  ON delivery_attempts (webhook_id, status, timestamp DESC);

CREATE INDEX IF NOT EXISTS idx_delivery_event_webhook
  ON delivery_attempts (event_id, webhook_id);

import express from "express";
import dotenv from "dotenv";
import { NotificationDatabase } from "./database";
import { PushNotificationService } from "./push-service";
import { FcmSenderConfig } from "./fcm-sender";
import { ApnsSenderConfig } from "./apns-sender";
import { ContractEventData } from "./event-triggers";
import { NotificationChannel, NotificationEventType } from "./types";

dotenv.config();

const app = express();
app.use(express.json());

const db = new NotificationDatabase(
  process.env.DATABASE_URL || "postgresql://localhost/notifications",
);

const fcmConfig: FcmSenderConfig | undefined = process.env.FCM_PROJECT_ID
  ? {
      projectId: process.env.FCM_PROJECT_ID,
      privateKey: (process.env.FCM_PRIVATE_KEY || "").replace(/\\n/g, "\n"),
      clientEmail: process.env.FCM_CLIENT_EMAIL || "",
    }
  : undefined;

const apnsConfig: ApnsSenderConfig | undefined = process.env.APNS_KEY_ID
  ? {
      keyId: process.env.APNS_KEY_ID,
      teamId: process.env.APNS_TEAM_ID || "",
      privateKey: (process.env.APNS_PRIVATE_KEY || "").replace(/\\n/g, "\n"),
      bundleId: process.env.APNS_BUNDLE_ID || "network.stellar.nebula.nomad",
      production: process.env.APNS_PRODUCTION === "true",
    }
  : undefined;

const notificationService = new PushNotificationService(
  db,
  fcmConfig,
  apnsConfig,
);

/**
 * Register a device for push notifications
 */
app.post("/devices", async (req, res) => {
  try {
    const { userId, deviceToken, platform } = req.body;
    if (!userId || !deviceToken || !platform) {
      return res.status(400).json({ error: "Missing required fields" });
    }
    if (!["ios", "android", "web"].includes(platform)) {
      return res.status(400).json({ error: "Invalid platform" });
    }
    await db.registerDevice(userId, deviceToken, platform);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Unregister a device
 */
app.delete("/devices/:token", async (req, res) => {
  try {
    await db.unregisterDevice(req.params.token);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Set notification preferences
 */
app.put("/preferences", async (req, res) => {
  try {
    const { userId, channel, eventType, enabled } = req.body;
    if (!userId || !channel || !eventType) {
      return res.status(400).json({ error: "Missing required fields" });
    }
    await notificationService
      .getPreferenceManager()
      .setPreference(userId, channel, eventType, enabled);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Set bulk notification preferences
 */
app.put("/preferences/bulk", async (req, res) => {
  try {
    const { userId, preferences } = req.body;
    if (!userId || !Array.isArray(preferences)) {
      return res.status(400).json({ error: "Invalid request body" });
    }
    await notificationService
      .getPreferenceManager()
      .setBulkPreferences(userId, preferences);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Disable a notification channel for a user
 */
app.put("/preferences/:channel/disable", async (req, res) => {
  try {
    const { userId } = req.body;
    const { channel } = req.params;
    if (!userId || !channel) {
      return res.status(400).json({ error: "Missing required fields" });
    }
    await notificationService
      .getPreferenceManager()
      .disableChannel(userId, channel as NotificationChannel);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Get notification preferences for a user
 */
app.get("/preferences/:userId", async (req, res) => {
  try {
    const preferences = await notificationService
      .getPreferenceManager()
      .getPreferences(req.params.userId);
    res.json({ preferences });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Enable all notifications for a user
 */
app.post("/preferences/:userId/enable-all", async (req, res) => {
  try {
    await notificationService
      .getPreferenceManager()
      .enableAll(req.params.userId);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Disable all notifications for a user
 */
app.post("/preferences/:userId/disable-all", async (req, res) => {
  try {
    await notificationService
      .getPreferenceManager()
      .disableAll(req.params.userId);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Process a contract event and trigger notifications
 */
app.post("/events", async (req, res) => {
  try {
    const { type, userId, data } = req.body;
    if (!type || !userId) {
      return res.status(400).json({ error: "Missing required fields" });
    }
    const event: ContractEventData = { type, userId, data: data ?? {} };
    await notificationService.processEvent(event);
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Send a direct push notification to a user
 */
app.post("/send", async (req, res) => {
  try {
    const { userId, title, body, data } = req.body;
    if (!userId || !title || !body) {
      return res.status(400).json({ error: "Missing required fields" });
    }
    await notificationService.sendToUser(userId, {
      title,
      body,
      data: data ?? {},
    });
    res.json({ success: true });
  } catch (error: any) {
    res.status(500).json({ error: error.message });
  }
});

/**
 * Health check
 */
app.get("/health", (_req, res) => {
  res.json({ status: "healthy" });
});

async function start() {
  await db.init();
  const PORT = process.env.PORT || 4000;
  app.listen(PORT, () => {
    console.log(`Notification service running on port ${PORT}`);
  });
}

start().catch(console.error);

export { app, notificationService, db };

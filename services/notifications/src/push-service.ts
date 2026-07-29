import { FcmSender, FcmSenderConfig } from "./fcm-sender";
import { ApnsSender, ApnsSenderConfig } from "./apns-sender";
import { EventTriggerHandler, ContractEventData } from "./event-triggers";
import { PreferenceManager } from "./preferences";
import { NotificationDatabase } from "./database";
import { DeviceRegistration, PushNotificationPayload } from "./types";

export class PushNotificationService {
  private fcmSender: FcmSender;
  private apnsSender: ApnsSender;
  private eventHandler: EventTriggerHandler;
  private preferenceManager: PreferenceManager;
  private db: NotificationDatabase;

  constructor(
    db: NotificationDatabase,
    fcmConfig?: FcmSenderConfig,
    apnsConfig?: ApnsSenderConfig,
  ) {
    this.db = db;
    this.fcmSender = new FcmSender(fcmConfig);
    this.apnsSender = new ApnsSender(apnsConfig);
    this.eventHandler = new EventTriggerHandler();
    this.preferenceManager = new PreferenceManager(db);
  }

  getEventTriggerHandler(): EventTriggerHandler {
    return this.eventHandler;
  }

  getPreferenceManager(): PreferenceManager {
    return this.preferenceManager;
  }

  async processEvent(event: ContractEventData): Promise<void> {
    const eventType = this.eventHandler.resolveEventType(event.type);
    if (!eventType) return;

    const isPushEnabled = await this.preferenceManager.isEventEnabled(
      event.userId,
      "push",
      eventType,
    );

    if (!isPushEnabled) return;

    const payload = this.eventHandler.resolveNotificationPayload(event);
    if (!payload) return;

    payload.data = {
      ...payload.data,
      event_type: eventType,
      timestamp: String(Date.now()),
    };

    const devices = await this.db.getActiveDevices(event.userId);
    if (devices.length === 0) return;

    await this.sendToDevices(devices, payload);
  }

  async sendToDevices(
    devices: DeviceRegistration[],
    payload: PushNotificationPayload,
  ): Promise<void> {
    const androidDevices = devices.filter((d) => d.platform === "android");
    const iosDevices = devices.filter((d) => d.platform === "ios");
    const webDevices = devices.filter((d) => d.platform === "web");

    const results = await Promise.all([
      this.fcmSender.sendMulticast(androidDevices, payload),
      this.apnsSender.sendMulticast(iosDevices, payload),
      this.fcmSender.sendMulticast(webDevices, payload),
    ]);

    const failedTokens: string[] = [];
    for (const result of results) {
      if (result.failure > 0) {
      }
    }
  }

  async sendToUser(
    userId: string,
    payload: PushNotificationPayload,
  ): Promise<void> {
    const isPushEnabled = await this.preferenceManager.isEventEnabled(
      userId,
      "push",
      "all",
    );
    if (!isPushEnabled) return;

    const devices = await this.db.getActiveDevices(userId);
    if (devices.length === 0) return;

    await this.sendToDevices(devices, payload);
  }

  async sendToAll(
    payload: PushNotificationPayload,
  ): Promise<void> {
    const devices = await this.db.getAllActiveDevices();
    if (devices.length === 0) return;

    await this.sendToDevices(devices, payload);
  }
}

import { createSign } from "crypto";
import { DeviceRegistration, PushNotificationPayload } from "./types";

export interface ApnsSenderConfig {
  keyId: string;
  teamId: string;
  privateKey: string;
  bundleId: string;
  production: boolean;
}

export class ApnsSender {
  private config: ApnsSenderConfig | null = null;

  constructor(config?: ApnsSenderConfig) {
    this.config = config ?? null;
  }

  configure(config: ApnsSenderConfig): void {
    this.config = config;
  }

  async sendPush(
    device: DeviceRegistration,
    payload: PushNotificationPayload,
  ): Promise<boolean> {
    if (!this.config) return false;
    if (device.platform !== "ios") return false;

    try {
      const token = await this.createDeviceToken();
      const host = this.config.production
        ? "api.push.apple.com"
        : "api.sandbox.push.apple.com";

      const aps: Record<string, any> = {
        alert: {
          title: payload.title,
          body: payload.body,
        },
      };
      if (payload.badge !== undefined) aps.badge = payload.badge;
      if (payload.sound) aps.sound = payload.sound;

      const body: Record<string, any> = { aps };
      if (payload.data) {
        for (const [key, value] of Object.entries(payload.data)) {
          body[key] = value;
        }
      }

      const response = await fetch(
        `https://${host}/3/device/${device.deviceToken}`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `bearer ${token}`,
            "apns-topic": this.config.bundleId,
            "apns-push-type": "alert",
          },
          body: JSON.stringify(body),
        },
      );

      if (!response.ok) {
        const body = await response.text();
        if (
          body.includes("Unregistered") ||
          body.includes("BadDeviceToken") ||
          body.includes("DeviceTokenNotForTopic")
        ) {
          return false;
        }
        throw new Error(`APNs send failed: ${response.status} ${body}`);
      }

      return true;
    } catch {
      return false;
    }
  }

  async sendMulticast(
    devices: DeviceRegistration[],
    payload: PushNotificationPayload,
  ): Promise<{ success: number; failure: number }> {
    let success = 0;
    let failure = 0;

    for (const device of devices) {
      if (device.platform !== "ios") continue;
      const ok = await this.sendPush(device, payload);
      if (ok) success++;
      else failure++;
    }

    return { success, failure };
  }

  private async createDeviceToken(): Promise<string> {
    if (!this.config) throw new Error("APNs sender not configured");

    const now = Math.floor(Date.now() / 1000);
    const header = { alg: "ES256", kid: this.config.keyId };
    const payload = {
      iss: this.config.teamId,
      iat: now,
      exp: now + 3600,
    };

    const base64 = (obj: any) =>
      Buffer.from(JSON.stringify(obj))
        .toString("base64")
        .replace(/=/g, "")
        .replace(/\+/g, "-")
        .replace(/\//g, "_");

    const signingInput = `${base64(header)}.${base64(payload)}`;
    const sign = createSign("SHA256");
    sign.update(signingInput);
    sign.end();
    const signature = sign
      .sign(this.config.privateKey)
      .toString("base64")
      .replace(/=/g, "")
      .replace(/\+/g, "-")
      .replace(/\//g, "_");

    return `${signingInput}.${signature}`;
  }
}

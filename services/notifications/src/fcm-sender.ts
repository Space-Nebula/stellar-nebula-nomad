import {
  DeviceRegistration,
  PushNotificationPayload,
} from "./types";

export interface FcmSenderConfig {
  projectId: string;
  privateKey: string;
  clientEmail: string;
}

export class FcmSender {
  private config: FcmSenderConfig | null = null;
  private accessToken: string | null = null;
  private tokenExpiry: number = 0;

  constructor(config?: FcmSenderConfig) {
    this.config = config ?? null;
  }

  configure(config: FcmSenderConfig): void {
    this.config = config;
  }

  async sendPush(
    device: DeviceRegistration,
    payload: PushNotificationPayload,
  ): Promise<boolean> {
    if (!this.config) return false;
    try {
      const token = await this.getAccessToken();
      const message: Record<string, any> = {
        to: device.deviceToken,
        notification: {
          title: payload.title,
          body: payload.body,
        },
        data: payload.data ?? {},
      };

      if (payload.imageUrl) {
        message.notification!.image = payload.imageUrl;
      }
      if (payload.badge !== undefined) {
        message.notification!.badge = payload.badge;
      }
      if (payload.sound) {
        message.notification!.sound = payload.sound;
      }

      const response = await fetch(
        `https://fcm.googleapis.com/v1/projects/${this.config.projectId}/messages:send`,
        {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${token}`,
          },
          body: JSON.stringify({
            message: {
              token: device.deviceToken,
              notification: {
                title: payload.title,
                body: payload.body,
              },
              data: payload.data ?? {},
            },
          }),
        },
      );

      if (!response.ok) {
        const body = await response.text();
        if (body.includes("UNREGISTERED") || body.includes("NOT_FOUND")) {
          return false;
        }
        throw new Error(`FCM send failed: ${response.status} ${body}`);
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
      const ok = await this.sendPush(device, payload);
      if (ok) success++;
      else failure++;
    }

    return { success, failure };
  }

  private async getAccessToken(): Promise<string> {
    if (this.accessToken && Date.now() < this.tokenExpiry) {
      return this.accessToken;
    }

    if (!this.config) {
      throw new Error("FCM sender not configured");
    }

    const jwt = await this.createAssertion();
    const response = await fetch(
      "https://oauth2.googleapis.com/token",
      {
        method: "POST",
        headers: { "Content-Type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams({
          grant_type: "urn:ietf:params:oauth:grant-type:jwt-bearer",
          assertion: jwt,
        }),
      },
    );

    const data: any = await response.json();
    this.accessToken = data.access_token;
    this.tokenExpiry = Date.now() + (data.expires_in - 60) * 1000;
    return this.accessToken!;
  }

  private async createAssertion(): Promise<string> {
    const { createSign } = await import("crypto");
    const header = { alg: "RS256", typ: "JWT" };
    const now = Math.floor(Date.now() / 1000);
    const payload = {
      iss: this.config!.clientEmail,
      scope: "https://www.googleapis.com/auth/firebase.messaging",
      aud: "https://oauth2.googleapis.com/token",
      exp: now + 3600,
      iat: now,
    };

    const base64 = (obj: any) =>
      Buffer.from(JSON.stringify(obj))
        .toString("base64")
        .replace(/=/g, "")
        .replace(/\+/g, "-")
        .replace(/\//g, "_");

    const signingInput = `${base64(header)}.${base64(payload)}`;
    const sign = createSign("RSA-SHA256");
    sign.update(signingInput);
    const signature = sign
      .sign(this.config!.privateKey)
      .toString("base64")
      .replace(/=/g, "")
      .replace(/\+/g, "-")
      .replace(/\//g, "_");

    return `${signingInput}.${signature}`;
  }
}

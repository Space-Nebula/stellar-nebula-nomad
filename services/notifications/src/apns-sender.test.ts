const mockFetch = jest.fn();
global.fetch = mockFetch as any;

import { ApnsSender } from "./apns-sender";

const TEST_EC_KEY = `-----BEGIN EC PRIVATE KEY-----
MHQCAQEEIIm3V2UfML6hUhwH5b0qGzq0qGzq0qGzq0qGzq0qGzq0oAcGBSuB
BAAi
-----END EC PRIVATE KEY-----`;

describe("ApnsSender", () => {
  let sender: ApnsSender;

  beforeEach(() => {
    jest.clearAllMocks();
    jest.spyOn(ApnsSender.prototype as any, "createDeviceToken")
      .mockResolvedValue("mock-apns-token");
    sender = new ApnsSender({
      keyId: "ABC123",
      teamId: "TEAM456",
      privateKey: TEST_EC_KEY,
      bundleId: "network.stellar.nebula.nomad",
      production: false,
    });
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  it("returns false when not configured", async () => {
    const unconfigured = new ApnsSender();
    const result = await unconfigured.sendPush(
      { userId: "u1", deviceToken: "token", platform: "ios" as const, active: true, createdAt: new Date(), updatedAt: new Date() },
      { title: "Test", body: "Body" },
    );
    expect(result).toBe(false);
  });

  it("skips non-iOS devices", async () => {
    const result = await sender.sendPush(
      {
        userId: "u1",
        deviceToken: "android-token",
        platform: "android",
        active: true,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
      { title: "Test", body: "Body" },
    );
    expect(result).toBe(false);
  });

  it("sends push notification to iOS device", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true, json: async () => ({}) });

    const result = await sender.sendPush(
      {
        userId: "u1",
        deviceToken: "ios-device-token",
        platform: "ios",
        active: true,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
      { title: "iOS Test", body: "Body", badge: 1, sound: "default" },
    );

    expect(result).toBe(true);
    expect(mockFetch).toHaveBeenCalledWith(
      expect.stringContaining("api.sandbox.push.apple.com"),
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          "apns-topic": "network.stellar.nebula.nomad",
          "apns-push-type": "alert",
        }),
      }),
    );
  });
});

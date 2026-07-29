const mockFetch = jest.fn();
global.fetch = mockFetch as any;

import { FcmSender } from "./fcm-sender";

const TEST_RSA_KEY = `-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA0gLx2C3e1sFqQK1vG5HySZwYZ/xO0IAAKuMZ7tMqI9Jb
Qk5xPm7Yg+7f6VWYb5H4CJv3Xt5F2pG8wA3kNm+3b3X0y4sGqP8RFfR
-----END RSA PRIVATE KEY-----`;

describe("FcmSender", () => {
  let sender: FcmSender;

  beforeEach(() => {
    jest.clearAllMocks();
    jest.spyOn(FcmSender.prototype as any, "getAccessToken")
      .mockResolvedValue("mock-access-token");
    sender = new FcmSender({
      projectId: "test-project",
      privateKey: TEST_RSA_KEY,
      clientEmail: "test@test-project.iam.gserviceaccount.com",
    });
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  it("returns false when not configured", async () => {
    const unconfigured = new FcmSender();
    const result = await unconfigured.sendPush(
      { userId: "u1", deviceToken: "token", platform: "android" as const, active: true, createdAt: new Date(), updatedAt: new Date() },
      { title: "Test", body: "Body" },
    );
    expect(result).toBe(false);
  });

  it("sends push notification to Android device", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true, json: async () => ({}) });

    const result = await sender.sendPush(
      {
        userId: "u1",
        deviceToken: "device-token-123",
        platform: "android",
        active: true,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
      { title: "Test Title", body: "Test Body", data: { key: "value" } },
    );

    expect(result).toBe(true);
    expect(mockFetch).toHaveBeenCalledTimes(1);
    expect(mockFetch).toHaveBeenCalledWith(
      "https://fcm.googleapis.com/v1/projects/test-project/messages:send",
      expect.objectContaining({
        method: "POST",
        headers: expect.objectContaining({
          Authorization: "Bearer mock-access-token",
        }),
      }),
    );
  });

  it("supports web devices", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true, json: async () => ({}) });

    const result = await sender.sendPush(
      {
        userId: "u1",
        deviceToken: "web-token",
        platform: "web",
        active: true,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
      { title: "Web Test", body: "Body" },
    );

    expect(result).toBe(true);
  });

  it("handles unregistered device response", async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 200,
      text: async () => '{"error":"UNREGISTERED"}',
    });

    const result = await sender.sendPush(
      {
        userId: "u1",
        deviceToken: "bad-token",
        platform: "android",
        active: true,
        createdAt: new Date(),
        updatedAt: new Date(),
      },
      { title: "Test", body: "Body" },
    );

    expect(result).toBe(false);
  });
});

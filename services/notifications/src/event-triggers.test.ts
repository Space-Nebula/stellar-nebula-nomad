import { EventTriggerHandler } from "./event-triggers";

describe("EventTriggerHandler", () => {
  let handler: EventTriggerHandler;

  beforeEach(() => {
    handler = new EventTriggerHandler();
  });

  it("resolves low_resources notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "low_res",
      userId: "user1",
      data: { resourceType: "Fuel", balance: 10, threshold: 50 },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Low Resources");
    expect(payload!.body).toContain("Fuel");
    expect(payload!.body).toContain("10");
  });

  it("resolves rare_discovery notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "rare_find",
      userId: "user1",
      data: {},
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Rare Discovery!");
    expect(payload!.data!.screen).toBe("scan");
  });

  it("resolves crafting_complete notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "craft_ok",
      userId: "user1",
      data: {},
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Crafting Complete");
    expect(payload!.data!.screen).toBe("shipyard");
  });

  it("resolves yield_claimed notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "yield_claimed",
      userId: "user1",
      data: { amount: "500" },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Yield Claimed");
    expect(payload!.body).toContain("500");
  });

  it("resolves ship_minted notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "ship_minted",
      userId: "user1",
      data: { shipId: "42" },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("New Ship Minted");
    expect(payload!.body).toContain("42");
  });

  it("resolves nebula_scanned notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "nebula_scanned",
      userId: "user1",
      data: { nebulaId: "7" },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Nebula Scan Complete");
  });

  it("resolves harvest_complete notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "harvest_ok",
      userId: "user1",
      data: { shipId: "3", amount: "100" },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Harvest Complete");
    expect(payload!.body).toContain("100");
  });

  it("resolves stake_matured notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "stake_matured",
      userId: "user1",
      data: { resourceType: "Minerals" },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Stake Matured");
  });

  it("resolves account_alert notification", () => {
    const payload = handler.resolveNotificationPayload({
      type: "account_alert",
      userId: "user1",
      data: { message: "Suspicious login detected" },
    });
    expect(payload).not.toBeNull();
    expect(payload!.title).toBe("Account Alert");
    expect(payload!.body).toContain("Suspicious login detected");
  });

  it("returns null for unknown event types", () => {
    const payload = handler.resolveNotificationPayload({
      type: "unknown_event",
      userId: "user1",
      data: {},
    });
    expect(payload).toBeNull();
  });
});

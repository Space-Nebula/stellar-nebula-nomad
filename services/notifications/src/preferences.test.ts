import { PreferenceManager } from "./preferences";
import { NotificationDatabase } from "./database";
import { NotificationPreference } from "./types";

jest.mock("./database");

describe("PreferenceManager", () => {
  let manager: PreferenceManager;
  let mockDb: jest.Mocked<NotificationDatabase>;

  beforeEach(() => {
    const MockDb = NotificationDatabase as jest.Mock;
    mockDb = new MockDb() as jest.Mocked<NotificationDatabase>;
    mockDb.setPreference = jest.fn();
    mockDb.getPreferences = jest.fn();
    mockDb.getDefaultPreferences = jest.fn();
    mockDb.isEventEnabled = jest.fn();
    manager = new PreferenceManager(mockDb);
  });

  it("sets a preference", async () => {
    await manager.setPreference("user1", "push", "rare_discovery", false);
    expect(mockDb.setPreference).toHaveBeenCalledWith(
      "user1",
      "push",
      "rare_discovery",
      false,
    );
  });

  it("sets bulk preferences", async () => {
    await manager.setBulkPreferences("user1", [
      { channel: "push", eventType: "rare_discovery", enabled: false },
      { channel: "push", eventType: "low_resources", enabled: true },
    ]);
    expect(mockDb.setPreference).toHaveBeenCalledTimes(2);
  });

  it("returns default preferences when none are stored", async () => {
    mockDb.getPreferences.mockResolvedValue([]);
    const defaults: NotificationPreference[] = [
      { userId: "user1", channel: "push", eventType: "rare_discovery", enabled: true, updatedAt: new Date() },
    ];
    mockDb.getDefaultPreferences.mockResolvedValue(defaults);

    const prefs = await manager.getPreferences("user1");
    expect(prefs).toEqual(defaults);
  });

  it("returns stored preferences when they exist", async () => {
    const stored: NotificationPreference[] = [
      { userId: "user1", channel: "push", eventType: "rare_discovery", enabled: false, updatedAt: new Date() },
    ];
    mockDb.getPreferences.mockResolvedValue(stored);

    const prefs = await manager.getPreferences("user1");
    expect(prefs).toEqual(stored);
    expect(mockDb.getDefaultPreferences).not.toHaveBeenCalled();
  });

  it("enables all notifications", async () => {
    const defaults: NotificationPreference[] = [
      { userId: "user1", channel: "push", eventType: "rare_discovery", enabled: true, updatedAt: new Date() },
      { userId: "user1", channel: "in_app", eventType: "rare_discovery", enabled: true, updatedAt: new Date() },
    ];
    mockDb.getDefaultPreferences.mockResolvedValue(defaults);

    await manager.enableAll("user1");
    expect(mockDb.setPreference).toHaveBeenCalledTimes(2);
    expect(mockDb.setPreference).toHaveBeenCalledWith("user1", "push", "rare_discovery", true);
  });

  it("disables all notifications", async () => {
    const defaults: NotificationPreference[] = [
      { userId: "user1", channel: "push", eventType: "rare_discovery", enabled: false, updatedAt: new Date() },
    ];
    mockDb.getDefaultPreferences.mockResolvedValue(defaults);

    await manager.disableAll("user1");
    expect(mockDb.setPreference).toHaveBeenCalledWith("user1", "push", "rare_discovery", false);
  });
});

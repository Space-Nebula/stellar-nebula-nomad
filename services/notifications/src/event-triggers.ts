import { PushNotificationPayload, NotificationEventType } from "./types";

export interface ContractEventData {
  type: string;
  userId: string;
  data: Record<string, any>;
}

const EVENT_TYPE_MAP: Record<string, NotificationEventType> = {
  low_res: "low_resources",
  rare_find: "rare_discovery",
  craft_ok: "crafting_complete",
  yield_claimed: "yield_claimed",
  ship_minted: "ship_minted",
  nebula_scanned: "nebula_scanned",
  harvest_ok: "harvest_complete",
  stake_matured: "stake_matured",
  account_alert: "account_alert",
};

export class EventTriggerHandler {
  resolveNotificationPayload(
    event: ContractEventData,
  ): PushNotificationPayload | null {
    const eventType = EVENT_TYPE_MAP[event.type];
    if (!eventType) return null;

    const builder = this.getPayloadBuilder(eventType);
    if (!builder) return null;

    return builder(event.data);
  }

  resolveEventType(rawType: string): NotificationEventType | null {
    return EVENT_TYPE_MAP[rawType] ?? null;
  }

  private getPayloadBuilder(
    eventType: NotificationEventType,
  ): ((data: Record<string, any>) => PushNotificationPayload) | null {
    const builders: Partial<Record<NotificationEventType, (data: Record<string, any>) => PushNotificationPayload>> = {
      low_resources: (data) => ({
        title: "Low Resources",
        body: `Your ${data.resourceType ?? "resources"} are running low (${data.balance ?? "?"}/${data.threshold ?? "?"}).`,
        data: { event: "low_resources", screen: "inventory" },
      }),
      rare_discovery: (_data) => ({
        title: "Rare Discovery!",
        body: "You discovered a rare nebula anomaly! Check it out now.",
        data: { event: "rare_discovery", screen: "scan" },
      }),
      crafting_complete: (_data) => ({
        title: "Crafting Complete",
        body: "Your ship crafting has finished. Board your new vessel!",
        data: { event: "crafting_complete", screen: "shipyard" },
      }),
      yield_claimed: (data) => ({
        title: "Yield Claimed",
        body: `You claimed ${data.amount ?? "0"} yield rewards.`,
        data: { event: "yield_claimed", screen: "staking" },
      }),
      ship_minted: (data) => ({
        title: "New Ship Minted",
        body: `Your new ship (ID: ${data.shipId ?? "unknown"}) has been minted!`,
        data: { event: "ship_minted", screen: "fleet" },
      }),
      nebula_scanned: (data) => ({
        title: "Nebula Scan Complete",
        body: `Nebula ${data.nebulaId ?? ""} scan results are ready.`,
        data: { event: "nebula_scanned", screen: "scan" },
      }),
      harvest_complete: (data) => ({
        title: "Harvest Complete",
        body: `Resource harvest from ship ${data.shipId ?? ""} is complete. +${data.amount ?? "0"} collected.`,
        data: { event: "harvest_complete", screen: "inventory" },
      }),
      stake_matured: (data) => ({
        title: "Stake Matured",
        body: `Your staking period for ${data.resourceType ?? "resources"} has matured. Claim your rewards!`,
        data: { event: "stake_matured", screen: "staking" },
      }),
      account_alert: (data) => ({
        title: "Account Alert",
        body: data.message ?? "There was an account-related notification.",
        data: { event: "account_alert", screen: "settings" },
      }),
    };

    return builders[eventType] ?? null;
  }

  getNotificationEventType(rawType: string): NotificationEventType | null {
    return EVENT_TYPE_MAP[rawType] ?? null;
  }
}

import {
  QueuedOperation,
  ConflictStrategy,
  OfflineConfig,
  DEFAULT_OFFLINE_CONFIG,
} from "./types";

export class ConflictResolver {
  private config: OfflineConfig;

  constructor(config: OfflineConfig = DEFAULT_OFFLINE_CONFIG) {
    this.config = config;
  }

  resolve(
    queued: QueuedOperation[],
  ): QueuedOperation[] {
    if (queued.length <= 1) return queued;

    const groups = this.groupByMethodAndArgs(queued);
    const resolved: QueuedOperation[] = [];

    for (const [, group] of groups) {
      if (group.length <= 1) {
        resolved.push(group[0]);
        continue;
      }

      const winner = this.selectWinner(group);
      resolved.push(winner);
    }

    return resolved;
  }

  private groupByMethodAndArgs(
    ops: QueuedOperation[],
  ): Map<string, QueuedOperation[]> {
    const groups = new Map<string, QueuedOperation[]>();
    for (const op of ops) {
      const key = `${op.method}:${this.safeStringify(op.args)}`;
      const existing = groups.get(key) ?? [];
      existing.push(op);
      groups.set(key, existing);
    }
    return groups;
  }

  private safeStringify(value: any): string {
    return JSON.stringify(value, (_key, val) =>
      typeof val === "bigint" ? val.toString() : val,
    );
  }

  private selectWinner(group: QueuedOperation[]): QueuedOperation {
    switch (this.config.conflictStrategy) {
      case "last-write-wins":
        return group.reduce((latest, op) =>
          op.timestamp > latest.timestamp ? op : latest,
        );
      case "server-priority":
        return group.reduce((earliest, op) =>
          op.timestamp < earliest.timestamp ? op : earliest,
        );
      default:
        return group[group.length - 1];
    }
  }

  setStrategy(strategy: ConflictStrategy): void {
    this.config.conflictStrategy = strategy;
  }
}

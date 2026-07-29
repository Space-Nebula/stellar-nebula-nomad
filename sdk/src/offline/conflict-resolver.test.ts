import { ConflictResolver } from "./conflict-resolver";
import { QueuedOperation } from "./types";

describe("ConflictResolver", () => {
  const baseOp = {
    id: "op1",
    args: [],
    callerSerialized: "keypair:GABC",
    status: "pending" as const,
    retryCount: 0,
  };

  it("resolves with last-write-wins by default", () => {
    const resolver = new ConflictResolver();
    const ops: QueuedOperation[] = [
      { ...baseOp, id: "op1", method: "mintShip", args: ["GABC", 0], timestamp: 100 },
      { ...baseOp, id: "op2", method: "mintShip", args: ["GABC", 0], timestamp: 200 },
    ];

    const resolved = resolver.resolve(ops);
    expect(resolved).toHaveLength(1);
    expect(resolved[0].id).toBe("op2");
  });

  it("keeps unique operations", () => {
    const resolver = new ConflictResolver();
    const ops: QueuedOperation[] = [
      { ...baseOp, id: "op1", method: "mintShip", args: ["GABC", 0], timestamp: 100 },
      { ...baseOp, id: "op2", method: "scanNebula", args: [BigInt(1)], timestamp: 200 },
    ];

    const resolved = resolver.resolve(ops);
    expect(resolved).toHaveLength(2);
  });

  it("uses server-priority when configured", () => {
    const resolver = new ConflictResolver({ conflictStrategy: "server-priority" } as any);
    const ops: QueuedOperation[] = [
      { ...baseOp, id: "op1", method: "mintShip", args: ["GABC", 0], timestamp: 100 },
      { ...baseOp, id: "op2", method: "mintShip", args: ["GABC", 0], timestamp: 200 },
    ];

    const resolved = resolver.resolve(ops);
    expect(resolved).toHaveLength(1);
    expect(resolved[0].id).toBe("op1");
  });
});

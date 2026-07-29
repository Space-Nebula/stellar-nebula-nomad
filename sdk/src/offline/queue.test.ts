import { OperationQueue } from "./queue";
import { InMemoryStorage } from "./storage";

describe("OperationQueue", () => {
  let queue: OperationQueue;
  let storage: InMemoryStorage;

  beforeEach(() => {
    storage = new InMemoryStorage();
    queue = new OperationQueue(storage);
  });

  it("enqueues and dequeues operations in FIFO order", async () => {
    await queue.enqueue({
      method: "mintShip",
      args: ["GABC", 0],
      callerSerialized: "keypair:GABC",
    });
    await queue.enqueue({
      method: "scanNebula",
      args: [BigInt(1)],
      callerSerialized: "keypair:GABC",
    });

    const first = await queue.dequeue();
    expect(first).not.toBeNull();
    expect(first!.method).toBe("mintShip");

    const second = await queue.dequeue();
    expect(second).not.toBeNull();
    expect(second!.method).toBe("scanNebula");

    const empty = await queue.dequeue();
    expect(empty).toBeNull();
  });

  it("returns correct pending count", async () => {
    expect(await queue.size()).toBe(0);

    await queue.enqueue({
      method: "mintShip",
      args: [],
      callerSerialized: "keypair:GABC",
    });
    expect(await queue.size()).toBe(1);

    await queue.enqueue({
      method: "scanNebula",
      args: [],
      callerSerialized: "keypair:GABC",
    });
    expect(await queue.size()).toBe(2);
  });

  it("removes operations", async () => {
    const op = await queue.enqueue({
      method: "mintShip",
      args: [],
      callerSerialized: "keypair:GABC",
    });
    expect(await queue.size()).toBe(1);

    await queue.remove(op.id);
    expect(await queue.size()).toBe(0);
  });

  it("clears all operations", async () => {
    await queue.enqueue({
      method: "mintShip",
      args: [],
      callerSerialized: "keypair:GABC",
    });
    await queue.enqueue({
      method: "scanNebula",
      args: [],
      callerSerialized: "keypair:GABC",
    });
    expect(await queue.size()).toBe(2);

    await queue.clear();
    expect(await queue.size()).toBe(0);
  });

  it("handles bigint args via JSON serialization", async () => {
    await queue.enqueue({
      method: "scanNebula",
      args: [BigInt("12345678901234567890")],
      callerSerialized: "keypair:GABC",
    });

    const op = await queue.dequeue();
    expect(op).not.toBeNull();
    expect(op!.args[0]).toBe(BigInt("12345678901234567890"));
  });
});

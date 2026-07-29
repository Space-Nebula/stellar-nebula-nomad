import axios from "axios";
import { EventStreamer } from "./event-streamer";
import { ContractEvent } from "./types";

jest.mock("axios");
const mockedPost = axios.post as jest.MockedFunction<typeof axios.post>;

function rpcResult(result: unknown) {
  return { data: { jsonrpc: "2.0", id: 1, result } };
}

function rawEvent(id: string, ledger: number) {
  return {
    id,
    contractId: "CCONTRACT",
    topic: ["nebula_scanned"],
    value: { seed: "42" },
    ledger,
    ledgerClosedAt: "2026-01-01T00:00:00Z",
  };
}

describe("EventStreamer", () => {
  afterEach(() => {
    jest.useRealTimers();
  });

  function makeStreamer(overrides: Partial<{
    onEvent: (event: ContractEvent) => void;
    onError: (error: Error, failures: number) => void;
  }> = {}) {
    const events: ContractEvent[] = [];
    const errors: Array<{ error: Error; failures: number }> = [];
    const streamer = new EventStreamer({
      rpcUrl: "http://rpc.local",
      contractIds: ["CCONTRACT"],
      pollIntervalMs: 1000,
      initialBackoffMs: 500,
      maxBackoffMs: 8000,
      onEvent: overrides.onEvent ?? ((e) => events.push(e)),
      onError:
        overrides.onError ??
        ((error, failures) => errors.push({ error, failures })),
    });
    return { streamer, events, errors };
  }

  it("anchors the first poll at the current ledger tip", async () => {
    jest.useFakeTimers();
    const { streamer } = makeStreamer();
    mockedPost
      .mockResolvedValueOnce(rpcResult({ sequence: 1234 }))
      .mockResolvedValueOnce(rpcResult({ events: [] }));

    await streamer.start();
    streamer.stop();

    expect(mockedPost).toHaveBeenCalledTimes(2);
    expect(mockedPost.mock.calls[0][1]).toMatchObject({
      method: "getLatestLedger",
    });
    expect(mockedPost.mock.calls[1][1]).toMatchObject({
      method: "getEvents",
      params: expect.objectContaining({ startLedger: 1234 }),
    });
  });

  it("delivers events in order and tracks the cursor", async () => {
    jest.useFakeTimers();
    const { streamer, events } = makeStreamer();
    mockedPost.mockResolvedValueOnce(
      rpcResult({
        events: [rawEvent("0001-0000", 1), rawEvent("0001-0001", 1)],
        cursor: "0001-0001",
      }),
    );

    await streamer.start(1);
    streamer.stop();

    expect(events.map((e) => e.id)).toEqual(["0001-0000", "0001-0001"]);
    expect(events[0]).toEqual({
      id: "0001-0000",
      contractId: "CCONTRACT",
      topics: ["nebula_scanned"],
      value: { seed: "42" },
      ledger: 1,
      ledgerClosedAt: "2026-01-01T00:00:00Z",
    });
    expect(streamer.lastCursor).toBe("0001-0001");
  });

  it("resumes subsequent polls from the cursor, not startLedger", async () => {
    jest.useFakeTimers();
    const { streamer } = makeStreamer();
    mockedPost
      .mockResolvedValueOnce(
        rpcResult({ events: [rawEvent("0001-0000", 1)], cursor: "0001-0000" }),
      )
      .mockResolvedValueOnce(rpcResult({ events: [] }));

    await streamer.start(1);
    await jest.advanceTimersByTimeAsync(1000);
    streamer.stop();

    expect(mockedPost).toHaveBeenCalledTimes(2);
    const secondParams = (mockedPost.mock.calls[1][1] as any).params;
    expect(secondParams.pagination.cursor).toBe("0001-0000");
    expect(secondParams.startLedger).toBeUndefined();
  });

  it("polls at the configured interval while healthy", async () => {
    jest.useFakeTimers();
    const { streamer } = makeStreamer();
    mockedPost.mockResolvedValue(rpcResult({ events: [] }));

    await streamer.start(1);
    expect(streamer.currentBackoffMs()).toBe(1000);
    await jest.advanceTimersByTimeAsync(3000);
    streamer.stop();

    expect(mockedPost.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  describe("reconnection", () => {
    it("backs off exponentially on repeated failures, up to the ceiling", async () => {
      jest.useFakeTimers();
      const { streamer, errors } = makeStreamer();
      mockedPost.mockRejectedValue(new Error("ECONNREFUSED"));

      await streamer.start(1);
      expect(errors).toHaveLength(1);
      expect(streamer.currentBackoffMs()).toBe(500);

      await jest.advanceTimersByTimeAsync(500);
      expect(errors).toHaveLength(2);
      expect(streamer.currentBackoffMs()).toBe(1000);

      await jest.advanceTimersByTimeAsync(1000);
      expect(streamer.currentBackoffMs()).toBe(2000);

      // Failures 4..10 — backoff caps at maxBackoffMs.
      for (let i = 0; i < 7; i++) {
        await jest.advanceTimersByTimeAsync(streamer.currentBackoffMs());
      }
      expect(streamer.currentBackoffMs()).toBe(8000);
      streamer.stop();
    });

    it("recovers after an outage and resumes from the last cursor", async () => {
      jest.useFakeTimers();
      const { streamer, events, errors } = makeStreamer();
      mockedPost
        .mockResolvedValueOnce(
          rpcResult({ events: [rawEvent("0001-0000", 1)], cursor: "0001-0000" }),
        )
        .mockRejectedValueOnce(new Error("timeout"))
        .mockResolvedValueOnce(
          rpcResult({ events: [rawEvent("0002-0000", 2)], cursor: "0002-0000" }),
        );

      await streamer.start(1);
      await jest.advanceTimersByTimeAsync(1000); // fails → backoff 500ms
      expect(errors).toHaveLength(1);

      await jest.advanceTimersByTimeAsync(500); // recovers
      streamer.stop();

      expect(events.map((e) => e.id)).toEqual(["0001-0000", "0002-0000"]);
      expect(streamer.currentBackoffMs()).toBe(1000); // reset to poll interval
      const recoveryParams = (mockedPost.mock.calls[2][1] as any).params;
      expect(recoveryParams.pagination.cursor).toBe("0001-0000");
    });

    it("treats a malformed RPC response as a failure", async () => {
      jest.useFakeTimers();
      const { streamer, errors } = makeStreamer();
      mockedPost.mockResolvedValueOnce({ data: {} });

      await streamer.start(1);
      streamer.stop();

      expect(errors).toHaveLength(1);
      expect(errors[0].error.message).toContain("Malformed getEvents");
    });
  });

  it("stops polling after stop()", async () => {
    jest.useFakeTimers();
    const { streamer } = makeStreamer();
    mockedPost.mockResolvedValue(rpcResult({ events: [] }));

    await streamer.start(1);
    streamer.stop();
    expect(streamer.isRunning).toBe(false);

    const calls = mockedPost.mock.calls.length;
    await jest.advanceTimersByTimeAsync(10_000);
    expect(mockedPost.mock.calls.length).toBe(calls);
  });

  it("ignores a second start() while running", async () => {
    jest.useFakeTimers();
    const { streamer } = makeStreamer();
    mockedPost.mockResolvedValue(rpcResult({ events: [] }));

    await streamer.start(1);
    const calls = mockedPost.mock.calls.length;
    await streamer.start(1);
    expect(mockedPost.mock.calls.length).toBe(calls);
    streamer.stop();
  });
});

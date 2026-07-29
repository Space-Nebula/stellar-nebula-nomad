import { useState, useEffect, useCallback } from "react";
import { OfflineState, DEFAULT_OFFLINE_CONFIG } from "../offline/types";
import { SyncEngine } from "../offline/sync-engine";
import { OperationQueue } from "../offline/queue";
import { ReadCache } from "../offline/cache";
import { DefaultNetworkStatusMonitor } from "../offline/network-status";
import { InMemoryStorage } from "../offline/storage";

export function useOfflineSync(
  syncEngine?: SyncEngine,
) {
  const [state, setState] = useState<OfflineState>({
    isOnline: true,
    queueLength: 0,
    lastSyncTimestamp: null,
    syncInProgress: false,
  });

  useEffect(() => {
    if (!syncEngine) return;
    const unsub = syncEngine.subscribe((newState) => {
      setState({ ...newState });
    });
    return unsub;
  }, [syncEngine]);

  const triggerSync = useCallback(async () => {
    if (syncEngine) {
      await syncEngine.sync();
    }
  }, [syncEngine]);

  return { ...state, triggerSync };
}

export function createDefaultOfflineSync() {
  const storage = new InMemoryStorage();
  const networkMonitor = new DefaultNetworkStatusMonitor();
  const queue = new OperationQueue(storage);
  const cache = new ReadCache(storage);
  const syncEngine = new SyncEngine(queue, cache, networkMonitor, storage);
  return { storage, networkMonitor, queue, cache, syncEngine };
}

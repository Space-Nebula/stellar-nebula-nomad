export interface OfflineStatusState {
  isOnline: boolean;
  lastChangedTimestamp: number | null;
}

export type OfflineStatusListener = (status: OfflineStatusState) => void;

export function createOfflineStatusMonitor(
  listener: OfflineStatusListener,
) {
  let isOnline = true;
  let lastChangedTimestamp: number | null = null;

  const updateStatus = (online: boolean) => {
    if (online === isOnline) return;
    isOnline = online;
    lastChangedTimestamp = Date.now();
    listener({ isOnline, lastChangedTimestamp });
  };

  const ping = async () => {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000);
      await fetch("https://www.google.com/generate_204", {
        mode: "no-cors",
        signal: controller.signal,
      });
      clearTimeout(timeoutId);
      updateStatus(true);
    } catch {
      updateStatus(false);
    }
  };

  const intervalId = setInterval(ping, 30000);
  ping();

  return {
    stop: () => {
      clearInterval(intervalId);
    },
    isOnline: () => isOnline,
    getStatus: (): OfflineStatusState => ({
      isOnline,
      lastChangedTimestamp,
    }),
  };
}

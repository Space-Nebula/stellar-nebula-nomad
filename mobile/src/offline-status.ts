import NetInfo from "@react-native-community/netinfo";

export interface OfflineStatusState {
  isOnline: boolean;
  lastChangedTimestamp: number | null;
}

export type OfflineStatusListener = (status: OfflineStatusState) => void;

export function createOfflineStatusMonitor(
  listener: OfflineStatusListener,
) {
  let lastChangedTimestamp: number | null = null;

  const unsubscribe = NetInfo.addEventListener((state) => {
    const isOnline = state.isConnected ?? true;
    lastChangedTimestamp = Date.now();
    listener({ isOnline, lastChangedTimestamp });
  });

  return {
    stop: () => {
      unsubscribe();
    },
    isOnline: async () => {
      const state = await NetInfo.fetch();
      return state.isConnected ?? true;
    },
  };
}

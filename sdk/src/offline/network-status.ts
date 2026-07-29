export type NetworkStatusListener = (isOnline: boolean) => void;

export interface NetworkStatusMonitor {
  isOnline(): boolean;
  subscribe(listener: NetworkStatusListener): () => void;
  start(): void;
  stop(): void;
}

export class DefaultNetworkStatusMonitor implements NetworkStatusMonitor {
  private _isOnline = true;
  private listeners = new Set<NetworkStatusListener>();
  private intervalId: ReturnType<typeof setInterval> | null = null;

  isOnline(): boolean {
    return this._isOnline;
  }

  subscribe(listener: NetworkStatusListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  start(): void {
    const g = globalThis as any;
    if (typeof g.navigator !== "undefined" && g.navigator.onLine !== undefined) {
      this._isOnline = g.navigator.onLine;
      if (typeof g.window !== "undefined") {
        g.window.addEventListener("online", this.handleOnline);
        g.window.addEventListener("offline", this.handleOffline);
      }
    }
    this.intervalId = setInterval(() => this.ping(), 30000);
  }

  stop(): void {
    const g = globalThis as any;
    if (typeof g.window !== "undefined") {
      g.window.removeEventListener("online", this.handleOnline);
      g.window.removeEventListener("offline", this.handleOffline);
    }
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }

  private handleOnline = (): void => {
    this.setOnline(true);
  };

  private handleOffline = (): void => {
    this.setOnline(false);
  };

  private async ping(): Promise<void> {
    try {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), 5000);
      await fetch("https://www.google.com/generate_204", {
        mode: "no-cors",
        signal: controller.signal,
      });
      clearTimeout(timeoutId);
      this.setOnline(true);
    } catch {
      this.setOnline(false);
    }
  }

  private setOnline(value: boolean): void {
    if (this._isOnline === value) return;
    this._isOnline = value;
    for (const listener of this.listeners) {
      listener(value);
    }
  }
}

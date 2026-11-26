/**
 * Client-side mirror of the on-chain health report (#117). Keep in sync with
 * src/health_monitor.rs (`HealthReport`, `HealthThresholds`, `check_health`).
 */

export type HealthStatus = 'Healthy' | 'Degraded' | 'Unhealthy';
export type HealthCheck = 'err_rate' | 'resource' | 'perf';

export interface OperationStats {
  operation: string;
  calls: number;
  errors: number;
  totalCost: number;
  maxCost: number;
  lastErrorAt: number;
}

export interface ResourceUsage {
  resource: string;
  used: number;
  capacity: number;
  peakUsed: number;
  updatedAt: number;
}

export interface HealthReport {
  status: HealthStatus;
  uptimeSecs: number;
  totalCalls: number;
  totalErrors: number;
  errorRateBps: number;
  avgCost: number;
  maxCost: number;
  worstResource: string;
  worstResourceBps: number;
  failedChecks: HealthCheck[];
  checkedAt: number;
  operations: OperationStats[];
  resources: ResourceUsage[];
}

export const CHECK_LABELS: Record<HealthCheck, string> = {
  err_rate: 'Error rate above threshold',
  resource: 'Resource usage above threshold',
  perf: 'Average cost above budget',
};

/** Mirrors `ratio_bps`: integer basis points, 0 when the whole is 0. */
export function ratioBps(part: number, whole: number): number {
  return whole === 0 ? 0 : Math.floor((part * 10_000) / whole);
}

/** 1234 bps -> "12.34%". */
export function formatBps(bps: number): string {
  return `${(bps / 100).toFixed(2)}%`;
}

/** 93784 -> "1d 2h 3m". */
export function formatUptime(secs: number): string {
  const d = Math.floor(secs / 86_400);
  const h = Math.floor((secs % 86_400) / 3_600);
  const m = Math.floor((secs % 3_600) / 60);
  if (d > 0) return `${d}d ${h}h ${m}m`;
  if (h > 0) return `${h}h ${m}m`;
  return `${m}m`;
}

import React from 'react';
import { CHECK_LABELS, HealthReport, HealthStatus, formatBps, formatUptime, ratioBps } from '../utils/health';

export interface HealthDashboardProps {
  /** Latest on-chain `check_health` report. */
  report: HealthReport;
  onRefresh: () => void;
}

// Text labels carry the meaning; colour is only a secondary cue.
const STATUS_STYLE: Record<HealthStatus, { color: string; label: string }> = {
  Healthy: { color: '#34D399', label: 'Healthy' },
  Degraded: { color: '#FBBF24', label: 'Degraded' },
  Unhealthy: { color: '#F87171', label: 'Unhealthy' },
};

/** Contract health: status, uptime, error rates, performance and resources (#117). */
export const HealthDashboard: React.FC<HealthDashboardProps> = ({ report, onRefresh }) => {
  const status = STATUS_STYLE[report.status];
  return (
    <section aria-label="Contract health" style={{ padding: 16, borderRadius: 8, background: '#0F172A', color: '#F8FAFC' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h2 style={{ margin: 0, fontSize: '1.1rem' }}>Contract health</h2>
        <button type="button" onClick={onRefresh}>
          Run health check
        </button>
      </header>

      <p role="status" style={{ fontWeight: 600, color: status.color }}>
        Status: {status.label}
      </p>

      {report.failedChecks.length > 0 && (
        <ul aria-label="Active alerts" style={{ background: '#1E293B', borderRadius: 6, padding: '8px 24px' }}>
          {report.failedChecks.map((c) => (
            <li key={c}>{CHECK_LABELS[c]}</li>
          ))}
        </ul>
      )}

      <dl style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(140px, 1fr))', gap: 8 }}>
        <div>
          <dt>Uptime</dt>
          <dd>{formatUptime(report.uptimeSecs)}</dd>
        </div>
        <div>
          <dt>Calls</dt>
          <dd>{report.totalCalls}</dd>
        </div>
        <div>
          <dt>Error rate</dt>
          <dd>{formatBps(report.errorRateBps)}</dd>
        </div>
        <div>
          <dt>Avg / max cost</dt>
          <dd>
            {report.avgCost} / {report.maxCost}
          </dd>
        </div>
      </dl>

      <h3 style={{ fontSize: '0.95rem' }}>Operations</h3>
      {report.operations.length === 0 ? (
        <p>No operations recorded yet.</p>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.85rem' }}>
          <thead>
            <tr>
              <th align="left">Operation</th>
              <th align="left">Calls</th>
              <th align="left">Errors</th>
              <th align="left">Error rate</th>
              <th align="left">Avg cost</th>
            </tr>
          </thead>
          <tbody>
            {report.operations.map((op) => (
              <tr key={op.operation} data-testid={`op-${op.operation}`}>
                <td>{op.operation}</td>
                <td>{op.calls}</td>
                <td>{op.errors}</td>
                <td>{formatBps(ratioBps(op.errors, op.calls))}</td>
                <td>{op.calls === 0 ? 0 : Math.floor(op.totalCost / op.calls)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <h3 style={{ fontSize: '0.95rem' }}>Resources</h3>
      {report.resources.length === 0 ? (
        <p>No resources tracked yet.</p>
      ) : (
        <ul style={{ listStyle: 'none', padding: 0 }}>
          {report.resources.map((r) => (
            <li key={r.resource}>
              <label>
                {r.resource}{' '}
                <meter min={0} max={r.capacity} value={r.used} aria-label={`${r.resource} usage`} /> {r.used}/{r.capacity} (
                {formatBps(ratioBps(r.used, r.capacity))}, peak {r.peakUsed})
              </label>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};

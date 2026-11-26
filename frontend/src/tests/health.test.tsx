import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { HealthDashboard } from '../views/HealthDashboard';
import { formatBps, formatUptime, HealthReport, ratioBps } from '../utils/health';

describe('health rule mirrors', () => {
  test('basis points match the contract integer maths', () => {
    expect(ratioBps(1, 20)).toBe(500);
    expect(ratioBps(1, 3)).toBe(3333);
    expect(ratioBps(5, 0)).toBe(0);
    expect(formatBps(9700)).toBe('97.00%');
  });

  test('uptime formatting', () => {
    expect(formatUptime(59)).toBe('0m');
    expect(formatUptime(3_660)).toBe('1h 1m');
    expect(formatUptime(93_784)).toBe('1d 2h 3m');
  });
});

describe('HealthDashboard', () => {
  const report: HealthReport = {
    status: 'Degraded',
    uptimeSecs: 7_200,
    totalCalls: 20,
    totalErrors: 1,
    errorRateBps: 500,
    avgCost: 110,
    maxCost: 300,
    worstResource: 'store',
    worstResourceBps: 4_000,
    failedChecks: ['err_rate'],
    checkedAt: 10_000,
    operations: [{ operation: 'craft', calls: 20, errors: 1, totalCost: 2_200, maxCost: 300, lastErrorAt: 9_000 }],
    resources: [{ resource: 'store', used: 40, capacity: 100, peakUsed: 96, updatedAt: 9_500 }],
  };

  test('shows status, alerts, metrics and resources', () => {
    const onRefresh = jest.fn();
    render(<HealthDashboard report={report} onRefresh={onRefresh} />);
    expect(screen.getByRole('status').textContent).toBe('Status: Degraded');
    expect(screen.getByText('Error rate above threshold')).toBeDefined();
    expect(screen.getByText('2h 0m')).toBeDefined();
    expect(screen.getByTestId('op-craft').textContent).toContain('5.00%');
    expect(screen.getByLabelText('store usage')).toBeDefined();
    fireEvent.click(screen.getByText('Run health check'));
    expect(onRefresh).toHaveBeenCalled();
  });

  test('healthy report has no alert list', () => {
    render(<HealthDashboard report={{ ...report, status: 'Healthy', failedChecks: [], operations: [], resources: [] }} onRefresh={jest.fn()} />);
    expect(screen.queryByLabelText('Active alerts')).toBeNull();
    expect(screen.getByText('No operations recorded yet.')).toBeDefined();
  });
});

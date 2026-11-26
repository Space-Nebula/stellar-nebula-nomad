import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { PortalNetwork } from '../views/PortalNetwork';
import { currentStability, PortalLink, travelBlocker, travelCost } from '../utils/portals';

const link = (over: Partial<PortalLink>): PortalLink => ({
  id: 0,
  sourceNebula: 1,
  targetNebula: 2,
  stability: 100,
  travelCost: 100,
  open: true,
  access: 'Public',
  owner: 'GOWNER',
  ...over,
});

describe('portal rule mirrors', () => {
  test('stability decay and travel cost match the contract', () => {
    expect(currentStability(100, 0, 10 * 86_400)).toBe(80);
    expect(currentStability(100, 0, 86_399)).toBe(100);
    expect(currentStability(100, 0, 60 * 86_400)).toBe(0);
    expect(travelCost(80)).toBe(125);
    expect(travelCost(0)).toBe(Infinity);
  });

  test('travel blockers follow access, state and stability', () => {
    expect(travelBlocker(link({}), 'GME', false)).toBeNull();
    expect(travelBlocker(link({ access: 'OwnerOnly' }), 'GME', false)).toBe('Owner only');
    expect(travelBlocker(link({ access: 'OwnerOnly' }), 'GOWNER', false)).toBeNull();
    expect(travelBlocker(link({ access: 'Allowlist' }), 'GME', true)).toBeNull();
    expect(travelBlocker(link({ open: false }), 'GME', false)).toBe('Closed');
    expect(travelBlocker(link({ stability: 9 }), 'GME', false)).toBe('Unstable');
  });
});

describe('PortalNetwork', () => {
  const links = [
    link({ id: 0, name: 'Orion Gate', tags: ['trade'] }),
    link({ id: 1, targetNebula: 3, access: 'Allowlist' }),
    link({ id: 2, sourceNebula: 2, targetNebula: 1, open: false, owner: 'GME' }),
  ];

  test('discovers outgoing portals and only allows permitted travel', () => {
    const onTravel = jest.fn();
    render(<PortalNetwork links={links} currentNebula={1} traveler="GME" onTravel={onTravel} />);
    const buttons = screen.getAllByText('Travel') as HTMLButtonElement[];
    expect(buttons.map((b) => b.disabled)).toEqual([false, true]);
    expect(screen.getByTestId('portal-0').textContent).toContain('Orion Gate');
    expect(screen.getByText('(Not on allowlist)')).toBeDefined();
    fireEvent.click(buttons[0]);
    expect(onTravel).toHaveBeenCalledWith(0);
    expect(screen.getAllByRole('row')).toHaveLength(4);
  });

  test('owners can reopen their closed portals', () => {
    const onToggle = jest.fn();
    render(<PortalNetwork links={links} currentNebula={9} traveler="GME" onTravel={jest.fn()} onToggleOpen={onToggle} />);
    expect(screen.getByText('No portals discovered from this nebula.')).toBeDefined();
    expect(screen.getByTestId('link-2').textContent).toContain('Closed');
    fireEvent.click(screen.getByText('Reopen'));
    expect(onToggle).toHaveBeenCalledWith(2, true);
    expect(screen.queryByText('Close')).toBeNull();
  });
});

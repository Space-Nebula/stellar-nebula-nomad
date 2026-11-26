import React from 'react';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { AlliancePanel, AlliancePanelProps } from '../views/AlliancePanel';
import { benefitsForLevel, canKick, needsAgreement, territoryCost } from '../utils/alliances';

describe('alliance rule mirrors', () => {
  test('benefits and territory cost match the contract', () => {
    expect(benefitsForLevel(1)).toEqual({ level: 1, memberCap: 50, territoryDiscountBps: 0 });
    expect(benefitsForLevel(3)).toEqual({ level: 3, memberCap: 60, territoryDiscountBps: 500 });
    expect(benefitsForLevel(40).memberCap).toBe(100);
    expect(territoryCost(1)).toBe(500);
    expect(territoryCost(11)).toBe(375);
  });

  test('rank and diplomacy rules', () => {
    expect(canKick('Officer', 'Member')).toBe(true);
    expect(canKick('Officer', 'Officer')).toBe(false);
    expect(canKick('Member', 'Member')).toBe(false);
    expect(needsAgreement('AtWar', 'Neutral')).toBe(true);
    expect(needsAgreement('Allied', 'AtWar')).toBe(false);
  });
});

describe('AlliancePanel', () => {
  const base: AlliancePanelProps = {
    name: 'Orion',
    level: 3,
    viewer: 'GLEAD',
    members: [
      { address: 'GLEAD', role: 'Leader', contribution: 500 },
      { address: 'GOFF', role: 'Officer', contribution: 100 },
      { address: 'GMEM', role: 'Member', contribution: 10 },
    ],
    relations: [{ allianceId: 7, name: 'Lyra', status: 'AtWar', incoming: 'Neutral' }],
    objectives: [
      { objectiveId: 0, kind: 'mine', target: 10, progress: 4, rewardXp: 1000, deadline: 500, completed: false },
      { objectiveId: 1, kind: 'scan', target: 5, progress: 5, rewardXp: 50, deadline: 500, completed: true },
    ],
    now: 100,
    onSetRole: jest.fn(),
    onKick: jest.fn(),
    onTransferLeadership: jest.fn(),
    onProposeRelation: jest.fn(),
    onContribute: jest.fn(),
  };

  test('leader manages members and accepts peace', () => {
    const props = { ...base, onSetRole: jest.fn(), onProposeRelation: jest.fn() };
    render(<AlliancePanel {...props} />);
    expect(screen.getByTestId('benefits').textContent).toContain('3/60');
    fireEvent.click(within(screen.getByTestId('member-GMEM')).getByText('Promote'));
    expect(props.onSetRole).toHaveBeenCalledWith('GMEM', 'Officer');
    expect(within(screen.getByTestId('member-GLEAD')).queryByText('Kick')).toBeNull();
    fireEvent.click(screen.getByText('Accept Neutral'));
    expect(props.onProposeRelation).toHaveBeenCalledWith(7, 'Neutral');
    expect(screen.getByText('Propose Allied')).toBeDefined();
  });

  test('officers can only kick lower ranks and nobody contributes to closed objectives', () => {
    const props = { ...base, viewer: 'GOFF', onKick: jest.fn(), onContribute: jest.fn() };
    render(<AlliancePanel {...props} />);
    expect(screen.queryByText('Promote')).toBeNull();
    expect(screen.getAllByText('Kick')).toHaveLength(1);
    fireEvent.click(screen.getByText('Kick'));
    expect(props.onKick).toHaveBeenCalledWith('GMEM');
    const buttons = screen.getAllByText('Contribute') as HTMLButtonElement[];
    expect(buttons.map((b) => b.disabled)).toEqual([false, true]);
    fireEvent.click(buttons[0]);
    expect(props.onContribute).toHaveBeenCalledWith(0);
  });
});

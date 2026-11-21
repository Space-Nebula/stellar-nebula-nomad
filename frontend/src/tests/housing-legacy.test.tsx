import React from 'react';
import { render, screen, fireEvent } from '@testing-library/react';
import { Housing, HousingAction, HouseData } from '../views/Housing';

const house: HouseData = {
  owner: 'GABCD1234567890XYZ',
  size: 'Medium',
  access: 'Public',
  wallTheme: 2,
  floorTheme: 3,
  lightingTheme: 4,
  furnitureCount: 1,
  totalVisits: 7,
  averageRating: 450,
  ratingCount: 3,
};

describe('Housing UI', () => {
  test('renders house overview and customization controls', () => {
    const actions: HousingAction[] = [];
    render(<Housing house={house} onAction={(a) => actions.push(a)} />);

    expect(screen.getByText('Player Housing')).toBeDefined();
    expect(screen.getByText('Medium House')).toBeDefined();
    expect(screen.getByLabelText('Wall theme')).toBeDefined();
    expect(screen.getByLabelText('Floor theme')).toBeDefined();
    expect(screen.getByLabelText('Lighting theme')).toBeDefined();

    fireEvent.change(screen.getByLabelText('Wall theme'), { target: { value: '9' } });
    fireEvent.click(screen.getByText('Save Customization'));
    expect(actions).toContainEqual({
      type: 'customize',
      payload: { wallTheme: 9, floorTheme: 3, lightingTheme: 4 },
    });
  });

  test('supports house access changes', () => {
    const actions: HousingAction[] = [];
    render(<Housing house={house} onAction={(a) => actions.push(a)} />);

    fireEvent.change(screen.getByLabelText('House access'), { target: { value: 'Private' } });
    expect(actions).toContainEqual({ type: 'access', payload: { access: 'Private' } });
  });

  test('allows purchasing and placing furniture', () => {
    const actions: HousingAction[] = [];
    render(
      <Housing
        house={house}
        furnitureCatalog={[
          { furnitureId: 1, name: 'Cozy Chair', category: 'Seating', rarity: 'Common', costAmount: 100, isFunctional: false },
        ]}
        placements={[{ furnitureId: 1, x: 10, y: 20, rotation: 90, layer: 1 }]}
        onAction={(a) => actions.push(a)}
      />
    );

    fireEvent.click(screen.getByText('Furniture'));
    expect(screen.getByText('Cozy Chair')).toBeDefined();
    fireEvent.click(screen.getByText('Purchase (100)'));
    expect(actions).toContainEqual({ type: 'purchase', payload: { furnitureId: 1 } });

    fireEvent.click(screen.getByText('Place'));
    fireEvent.change(screen.getByLabelText('Furniture X'), { target: { value: '42' } });
    fireEvent.click(screen.getByText('Confirm Placement'));
    expect(actions).toContainEqual({
      type: 'place',
      payload: { furnitureId: 1, x: 42, y: 0, rotation: 0, layer: 1 },
    });

    fireEvent.click(screen.getByText('Remove'));
    expect(actions).toContainEqual({ type: 'remove-furniture', payload: { furnitureId: 1 } });
  });

  test('displays and removes achievements', () => {
    const actions: HousingAction[] = [];
    render(
      <Housing
        house={house}
        achievements={[{ displayId: 5, achievementId: 9, title: 'First Scan', slot: 1, installedAt: 1 }]}
        unlockedAchievements={[{ achievementId: 12, title: 'Nebula Explorer' }]}
        onAction={(a) => actions.push(a)}
      />
    );

    fireEvent.click(screen.getByText('Achievements'));
    expect(screen.getByText(/Slot 1: First Scan/)).toBeDefined();

    fireEvent.click(screen.getByText('Display in Case'));
    expect(actions).toContainEqual({ type: 'display-achievement', payload: { achievementId: 12, slot: 0 } });

    fireEvent.click(screen.getByText('Remove'));
    expect(actions).toContainEqual({ type: 'remove-achievement', payload: { displayId: 5 } });
  });

  test('sets and clears the guild banner', () => {
    const actions: HousingAction[] = [];
    render(<Housing house={house} onAction={(a) => actions.push(a)} />);

    fireEvent.click(screen.getByText('Guild Banner'));
    fireEvent.change(screen.getByLabelText('Guild banner motto'), { target: { value: 'Ad Astra' } });
    fireEvent.click(screen.getByText('Set Banner'));
    expect(actions).toContainEqual({ type: 'set-guild-banner', payload: { motto: 'Ad Astra' } });

    fireEvent.click(screen.getByText('Clear Banner'));
    expect(actions).toContainEqual({ type: 'clear-guild-banner' });
  });

  test('shows storage capacity and expands storage', () => {
    const actions: HousingAction[] = [];
    render(
      <Housing
        house={house}
        storage={{ level: 1, baseCapacity: 100, bonusCapacity: 100, capacity: 200, nextCost: 8000, maxLevel: 5 }}
        onAction={(a) => actions.push(a)}
      />
    );

    fireEvent.click(screen.getByText('Storage'));
    expect(screen.getByText(/Level 1 \/ 5 — capacity 200/)).toBeDefined();
    fireEvent.click(screen.getByText(/Expand Storage/));
    expect(actions).toContainEqual({ type: 'expand-storage' });
  });

  test('visits a house and lists visitors', () => {
    const actions: HousingAction[] = [];
    render(
      <Housing
        house={house}
        visitors={[{ visitor: 'GXYZ9876543210', visitedAt: 1700000000, leftRating: true }]}
        onAction={(a) => actions.push(a)}
      />
    );

    fireEvent.click(screen.getByText('Visitors'));
    fireEvent.change(screen.getByLabelText('House owner address'), { target: { value: 'GOWNER123' } });
    fireEvent.click(screen.getByText('Visit'));
    expect(actions).toContainEqual({ type: 'visit', payload: { houseOwner: 'GOWNER123' } });
    expect(screen.getByText('Rated ★')).toBeDefined();
  });
});

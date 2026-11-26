import React, { useState } from 'react';
import './ShipManagement.css';

export interface ShipStats {
  attack: number;
  defense: number;
  speed: number;
  health: number;
  capacity: number;
}

export interface ShipAbility {
  id: number;
  name: string;
  cooldown: number;
  description: string;
  unlocked: boolean;
}

export interface Ship {
  id: number;
  name: string;
  type: string;
  level: number;
  experience: number;
  stats: ShipStats;
  abilities: ShipAbility[];
  durability: number;
  maxDurability: number;
  customizationLevel: number;
  rarity: 'common' | 'uncommon' | 'rare' | 'epic' | 'legendary';
}

export interface ShipManagementProps {
  ships: Ship[];
  selectedShipId?: number;
  onSelectShip?: (shipId: number) => void;
  onUpgrade?: (shipId: number) => void;
  onRepair?: (shipId: number) => void;
  onCustomize?: (shipId: number) => void;
  onAbilityUnlock?: (shipId: number, abilityId: number) => void;
}

export const ShipManagement: React.FC<ShipManagementProps> = ({
  ships,
  selectedShipId,
  onSelectShip,
  onUpgrade,
  onRepair,
  onCustomize,
  onAbilityUnlock,
}) => {
  const [activeTab, setActiveTab] = useState<'overview' | 'upgrade' | 'repair' | 'customize' | 'abilities'>('overview');
  const selectedShip = ships.find(s => s.id === selectedShipId) || ships[0];

  const getRarityColor = (rarity: string) => {
    const colors: Record<string, string> = {
      common: '#888888',
      uncommon: '#4a9eff',
      rare: '#a335ee',
      epic: '#ff8000',
      legendary: '#ff0070',
    };
    return colors[rarity] || '#ffffff';
  };

  const UpgradeTab = () => (
    <div className="ship-tab">
      <h3>Ship Upgrades</h3>
      <div className="upgrade-container">
        <div className="upgrade-item">
          <span>Level {selectedShip.level}</span>
          <div className="upgrade-bar">
            <div className="upgrade-progress" style={{ width: `${(selectedShip.experience % 100)}%` }}></div>
          </div>
          <button onClick={() => onUpgrade?.(selectedShip.id)} className="upgrade-btn">Upgrade</button>
        </div>
      </div>
    </div>
  );

  const RepairTab = () => (
    <div className="ship-tab">
      <h3>Ship Repair</h3>
      <div className="repair-container">
        <div className="durability-status">
          <div className="durability-bar">
            <div
              className={`durability-fill ${selectedShip.durability < selectedShip.maxDurability / 3 ? 'critical' : ''}`}
              style={{ width: `${(selectedShip.durability / selectedShip.maxDurability) * 100}%` }}
            ></div>
          </div>
          <span>{selectedShip.durability} / {selectedShip.maxDurability}</span>
        </div>
        <button
          onClick={() => onRepair?.(selectedShip.id)}
          className="repair-btn"
          disabled={selectedShip.durability === selectedShip.maxDurability}
        >
          Repair
        </button>
      </div>
    </div>
  );

  const CustomizeTab = () => (
    <div className="ship-tab">
      <h3>Ship Customization</h3>
      <div className="customize-container">
        <div className="customization-level">
          <span>Customization Level: {selectedShip.customizationLevel}</span>
          <div className="customization-slots">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className={`slot ${i < selectedShip.customizationLevel ? 'active' : ''}`}></div>
            ))}
          </div>
        </div>
        <button onClick={() => onCustomize?.(selectedShip.id)} className="customize-btn">Customize</button>
      </div>
    </div>
  );

  const AbilitiesTab = () => (
    <div className="ship-tab">
      <h3>Ship Abilities</h3>
      <div className="abilities-grid">
        {selectedShip.abilities.map(ability => (
          <div key={ability.id} className={`ability-card ${ability.unlocked ? 'unlocked' : 'locked'}`}>
            <div className="ability-name">{ability.name}</div>
            <div className="ability-description">{ability.description}</div>
            <div className="ability-cooldown">Cooldown: {ability.cooldown}s</div>
            {!ability.unlocked && (
              <button
                onClick={() => onAbilityUnlock?.(selectedShip.id, ability.id)}
                className="unlock-btn"
              >
                Unlock
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );

  return (
    <div className="ship-management">
      <div className="ship-container">
        <div className="ship-list">
          <h2>Your Fleet</h2>
          <div className="ships-grid">
            {ships.map(ship => (
              <div
                key={ship.id}
                className={`ship-card ${selectedShipId === ship.id ? 'selected' : ''}`}
                onClick={() => onSelectShip?.(ship.id)}
                style={{ borderColor: getRarityColor(ship.rarity) }}
              >
                <div className="ship-name">{ship.name}</div>
                <div className="ship-type">{ship.type}</div>
                <div className="ship-level">Lv. {ship.level}</div>
              </div>
            ))}
          </div>
        </div>

        <div className="ship-details">
          <div className="ship-header">
            <h1 style={{ color: getRarityColor(selectedShip.rarity) }}>{selectedShip.name}</h1>
            <div className="ship-info">
              <span className="ship-type">{selectedShip.type}</span>
              <span className="ship-rarity" style={{ color: getRarityColor(selectedShip.rarity) }}>
                {selectedShip.rarity.toUpperCase()}
              </span>
            </div>
          </div>

          <div className="ship-stats">
            <div className="stat">
              <span className="stat-name">Attack</span>
              <div className="stat-bar">
                <div className="stat-fill" style={{ width: `${Math.min(selectedShip.stats.attack / 10 * 100, 100)}%` }}></div>
              </div>
              <span className="stat-value">{selectedShip.stats.attack}</span>
            </div>
            <div className="stat">
              <span className="stat-name">Defense</span>
              <div className="stat-bar">
                <div className="stat-fill" style={{ width: `${Math.min(selectedShip.stats.defense / 10 * 100, 100)}%` }}></div>
              </div>
              <span className="stat-value">{selectedShip.stats.defense}</span>
            </div>
            <div className="stat">
              <span className="stat-name">Speed</span>
              <div className="stat-bar">
                <div className="stat-fill" style={{ width: `${Math.min(selectedShip.stats.speed / 10 * 100, 100)}%` }}></div>
              </div>
              <span className="stat-value">{selectedShip.stats.speed}</span>
            </div>
            <div className="stat">
              <span className="stat-name">Health</span>
              <div className="stat-bar">
                <div className="stat-fill" style={{ width: `${Math.min(selectedShip.stats.health / 10 * 100, 100)}%` }}></div>
              </div>
              <span className="stat-value">{selectedShip.stats.health}</span>
            </div>
            <div className="stat">
              <span className="stat-name">Capacity</span>
              <div className="stat-bar">
                <div className="stat-fill" style={{ width: `${Math.min(selectedShip.stats.capacity / 10 * 100, 100)}%` }}></div>
              </div>
              <span className="stat-value">{selectedShip.stats.capacity}</span>
            </div>
          </div>

          <div className="tabs">
            <button
              className={`tab-btn ${activeTab === 'overview' ? 'active' : ''}`}
              onClick={() => setActiveTab('overview')}
            >
              Overview
            </button>
            <button
              className={`tab-btn ${activeTab === 'upgrade' ? 'active' : ''}`}
              onClick={() => setActiveTab('upgrade')}
            >
              Upgrade
            </button>
            <button
              className={`tab-btn ${activeTab === 'repair' ? 'active' : ''}`}
              onClick={() => setActiveTab('repair')}
            >
              Repair
            </button>
            <button
              className={`tab-btn ${activeTab === 'customize' ? 'active' : ''}`}
              onClick={() => setActiveTab('customize')}
            >
              Customize
            </button>
            <button
              className={`tab-btn ${activeTab === 'abilities' ? 'active' : ''}`}
              onClick={() => setActiveTab('abilities')}
            >
              Abilities
            </button>
          </div>

          <div className="tab-content">
            {activeTab === 'overview' && (
              <div className="ship-tab">
                <h3>Ship Overview</h3>
                <div className="overview-info">
                  <p><strong>Type:</strong> {selectedShip.type}</p>
                  <p><strong>Level:</strong> {selectedShip.level}</p>
                  <p><strong>Experience:</strong> {selectedShip.experience}</p>
                  <p><strong>Customization:</strong> Level {selectedShip.customizationLevel}</p>
                </div>
              </div>
            )}
            {activeTab === 'upgrade' && <UpgradeTab />}
            {activeTab === 'repair' && <RepairTab />}
            {activeTab === 'customize' && <CustomizeTab />}
            {activeTab === 'abilities' && <AbilitiesTab />}
          </div>
        </div>
      </div>
    </div>
  );
};

export default ShipManagement;

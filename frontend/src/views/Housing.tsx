import React, { useState } from 'react';

// ─── On-chain shapes (mirrors src/player_housing.rs) ──────────────────────────

export type HouseSize = 'Small' | 'Medium' | 'Large';
export type HouseAccess = 'Public' | 'Private' | 'FriendsOnly';

export interface HouseData {
  owner: string;
  size: HouseSize;
  access: HouseAccess;
  wallTheme: number;
  floorTheme: number;
  lightingTheme: number;
  furnitureCount: number;
  totalVisits: number;
  averageRating: number;
  ratingCount: number;
}

export interface PlacedFurnitureData {
  furnitureId: number;
  name?: string;
  x: number;
  y: number;
  rotation: number;
  layer: number;
}

export interface FurnitureItemData {
  furnitureId: number;
  name: string;
  category: string;
  rarity: string;
  costAmount: number;
  isFunctional: boolean;
  bonusDescription?: string;
}

export interface AchievementDisplayData {
  displayId: number;
  achievementId: number;
  title?: string;
  slot: number;
  installedAt: number;
}

export interface UnlockedAchievementData {
  achievementId: number;
  title: string;
  category?: string;
}

export interface GuildBannerData {
  guildName: string;
  motto: string;
  installedAt: number;
}

export interface HouseStorageData {
  level: number;
  baseCapacity: number;
  bonusCapacity: number;
  capacity: number;
  nextCost?: number;
  maxLevel: number;
}

export interface VisitorData {
  visitor: string;
  visitedAt: number;
  leftRating: boolean;
}

export interface HousingAction {
  type:
    | 'customize'
    | 'access'
    | 'purchase'
    | 'place'
    | 'remove-furniture'
    | 'display-achievement'
    | 'remove-achievement'
    | 'set-guild-banner'
    | 'clear-guild-banner'
    | 'expand-storage'
    | 'visit';
  payload?: Record<string, unknown>;
}

export interface HousingProps {
  house?: HouseData | null;
  placements?: PlacedFurnitureData[];
  furnitureCatalog?: FurnitureItemData[];
  achievements?: AchievementDisplayData[];
  unlockedAchievements?: UnlockedAchievementData[];
  guildBanner?: GuildBannerData | null;
  storage?: HouseStorageData | null;
  visitors?: VisitorData[];
  onAction?: (action: HousingAction) => void;
}

const panelStyle: React.CSSProperties = {
  background: '#0F172A',
  border: '1px solid #334155',
  borderRadius: '8px',
  padding: '16px',
};

const inputStyle: React.CSSProperties = {
  background: '#1E293B',
  border: '1px solid #475569',
  borderRadius: '4px',
  color: '#E2E8F0',
  padding: '6px 8px',
  width: '100%',
};

const buttonStyle: React.CSSProperties = {
  background: '#3B82F6',
  border: 'none',
  borderRadius: '4px',
  color: '#FFFFFF',
  cursor: 'pointer',
  padding: '6px 12px',
};

const secondaryButtonStyle: React.CSSProperties = {
  ...buttonStyle,
  background: '#334155',
};

type Tab = 'overview' | 'furniture' | 'achievements' | 'guild' | 'storage' | 'visitors';

const TABS: { id: Tab; label: string }[] = [
  { id: 'overview', label: 'Overview' },
  { id: 'furniture', label: 'Furniture' },
  { id: 'achievements', label: 'Achievements' },
  { id: 'guild', label: 'Guild Banner' },
  { id: 'storage', label: 'Storage' },
  { id: 'visitors', label: 'Visitors' },
];

function shortAddress(address: string): string {
  return address.length > 14 ? `${address.slice(0, 7)}…${address.slice(-5)}` : address;
}

function formatDate(unixSeconds: number): string {
  if (!unixSeconds) return '—';
  return new Date(unixSeconds * 1000).toISOString().slice(0, 16).replace('T', ' ');
}

/** Housing UI — house customization, furniture, displays, guild banner, storage and visitors. */
export const Housing: React.FC<HousingProps> = ({
  house = null,
  placements = [],
  furnitureCatalog = [],
  achievements = [],
  unlockedAchievements = [],
  guildBanner = null,
  storage = null,
  visitors = [],
  onAction,
}) => {
  const [activeTab, setActiveTab] = useState<Tab>('overview');
  const [wallTheme, setWallTheme] = useState(house?.wallTheme ?? 1);
  const [floorTheme, setFloorTheme] = useState(house?.floorTheme ?? 1);
  const [lightingTheme, setLightingTheme] = useState(house?.lightingTheme ?? 1);
  const [motto, setMotto] = useState(guildBanner?.motto ?? '');
  const [placementTarget, setPlacementTarget] = useState<number | null>(null);
  const [posX, setPosX] = useState(0);
  const [posY, setPosY] = useState(0);
  const [rotation, setRotation] = useState(0);
  const [layer, setLayer] = useState(1);
  const [achievementSlot, setAchievementSlot] = useState(0);
  const [visitorAddress, setVisitorAddress] = useState('');

  const emit = (action: HousingAction) => onAction?.(action);

  return (
    <section style={{ color: '#E2E8F0', display: 'grid', gap: '16px' }} aria-label="Player Housing">
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 style={{ margin: 0 }}>Player Housing</h2>
          <p style={{ margin: '4px 0', color: '#94A3B8', fontSize: '0.85rem' }}>
            {house ? `Home of ${shortAddress(house.owner)}` : 'No house created yet'}
          </p>
        </div>
        <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
          {house && <span style={{ ...panelStyle, padding: '6px 10px' }}>{house.size} House</span>}
          {house && (
            <select
              aria-label="House access"
              value={house.access}
              style={inputStyle}
              onChange={(e) =>
                emit({ type: 'access', payload: { access: e.target.value as HouseAccess } })
              }
            >
              <option value="Public">Public</option>
              <option value="Private">Private</option>
              <option value="FriendsOnly">Friends Only</option>
            </select>
          )}
        </div>
      </header>

      <nav style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }} aria-label="Housing sections">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            type="button"
            aria-pressed={activeTab === tab.id}
            onClick={() => setActiveTab(tab.id)}
            style={{
              ...(activeTab === tab.id ? buttonStyle : secondaryButtonStyle),
              opacity: activeTab === tab.id ? 1 : 0.75,
            }}
          >
            {tab.label}
          </button>
        ))}
      </nav>

      {activeTab === 'overview' && (
        <div style={{ display: 'grid', gap: '12px' }}>
          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>House Customization</h3>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '8px' }}>
              <label>
                Wall theme
                <input
                  aria-label="Wall theme"
                  type="number"
                  min={0}
                  value={wallTheme}
                  onChange={(e) => setWallTheme(Number(e.target.value))}
                  style={inputStyle}
                />
              </label>
              <label>
                Floor theme
                <input
                  aria-label="Floor theme"
                  type="number"
                  min={0}
                  value={floorTheme}
                  onChange={(e) => setFloorTheme(Number(e.target.value))}
                  style={inputStyle}
                />
              </label>
              <label>
                Lighting theme
                <input
                  aria-label="Lighting theme"
                  type="number"
                  min={0}
                  value={lightingTheme}
                  onChange={(e) => setLightingTheme(Number(e.target.value))}
                  style={inputStyle}
                />
              </label>
            </div>
            <button
              type="button"
              style={{ ...buttonStyle, marginTop: '12px' }}
              onClick={() =>
                emit({
                  type: 'customize',
                  payload: { wallTheme, floorTheme, lightingTheme },
                })
              }
            >
              Save Customization
            </button>
          </div>

          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(140px, 1fr))', gap: '12px' }}>
            <div style={panelStyle}>
              <strong>{house?.furnitureCount ?? 0}</strong>
              <p style={{ margin: 0, color: '#94A3B8' }}>Furniture placed</p>
            </div>
            <div style={panelStyle}>
              <strong>{house?.totalVisits ?? 0}</strong>
              <p style={{ margin: 0, color: '#94A3B8' }}>Total visits</p>
            </div>
            <div style={panelStyle}>
              <strong>{house ? (house.averageRating / 100).toFixed(1) : '—'} ★</strong>
              <p style={{ margin: 0, color: '#94A3B8' }}>
                Rating ({house?.ratingCount ?? 0})
              </p>
            </div>
          </div>
        </div>
      )}

      {activeTab === 'furniture' && (
        <div style={{ display: 'grid', gap: '12px' }}>
          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>Furniture Catalog</h3>
            {furnitureCatalog.length === 0 && <p>No furniture catalog items available.</p>}
            <div style={{ display: 'grid', gap: '8px' }}>
              {furnitureCatalog.map((item) => (
                <div
                  key={item.furnitureId}
                  style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }}
                >
                  <span>
                    <strong>{item.name}</strong> — {item.category} / {item.rarity}
                    {item.isFunctional && ` (${item.bonusDescription || 'functional'})`}
                  </span>
                  <span style={{ display: 'flex', gap: '6px' }}>
                    <button
                      type="button"
                      style={secondaryButtonStyle}
                      onClick={() => emit({ type: 'purchase', payload: { furnitureId: item.furnitureId } })}
                    >
                      Purchase ({item.costAmount})
                    </button>
                    <button
                      type="button"
                      style={buttonStyle}
                      onClick={() => setPlacementTarget(item.furnitureId)}
                    >
                      Place
                    </button>
                  </span>
                </div>
              ))}
            </div>
            {placementTarget !== null && (
              <div style={{ marginTop: '12px', display: 'flex', flexWrap: 'wrap', gap: '8px', alignItems: 'end' }}>
                <label>
                  X
                  <input aria-label="Furniture X" type="number" min={0} max={999} value={posX} onChange={(e) => setPosX(Number(e.target.value))} style={inputStyle} />
                </label>
                <label>
                  Y
                  <input aria-label="Furniture Y" type="number" min={0} max={999} value={posY} onChange={(e) => setPosY(Number(e.target.value))} style={inputStyle} />
                </label>
                <label>
                  Rotation
                  <select aria-label="Furniture rotation" value={rotation} onChange={(e) => setRotation(Number(e.target.value))} style={inputStyle}>
                    {[0, 90, 180, 270].map((r) => (
                      <option key={r} value={r}>{r}°</option>
                    ))}
                  </select>
                </label>
                <label>
                  Layer
                  <input aria-label="Furniture layer" type="number" min={0} value={layer} onChange={(e) => setLayer(Number(e.target.value))} style={inputStyle} />
                </label>
                <button
                  type="button"
                  style={buttonStyle}
                  onClick={() => {
                    emit({
                      type: 'place',
                      payload: { furnitureId: placementTarget, x: posX, y: posY, rotation, layer },
                    });
                    setPlacementTarget(null);
                  }}
                >
                  Confirm Placement
                </button>
              </div>
            )}
          </div>

          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>Placed Furniture ({placements.length})</h3>
            {placements.length === 0 && <p>No furniture placed yet.</p>}
            <div style={{ display: 'grid', gap: '6px' }}>
              {placements.map((p, index) => (
                <div key={`${p.furnitureId}-${index}`} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <span>
                    {p.name || `Furniture #${p.furnitureId}`} — ({p.x}, {p.y}) {p.rotation}° layer {p.layer}
                  </span>
                  <button
                    type="button"
                    style={secondaryButtonStyle}
                    onClick={() => emit({ type: 'remove-furniture', payload: { furnitureId: p.furnitureId } })}
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {activeTab === 'achievements' && (
        <div style={{ display: 'grid', gap: '12px' }}>
          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>Achievement Display Cases</h3>
            {achievements.length === 0 && <p>No achievements on display.</p>}
            <div style={{ display: 'grid', gap: '6px' }}>
              {achievements.map((a) => (
                <div key={a.displayId} style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span>
                    Slot {a.slot}: {a.title || `Achievement #${a.achievementId}`}
                  </span>
                  <button
                    type="button"
                    style={secondaryButtonStyle}
                    onClick={() => emit({ type: 'remove-achievement', payload: { displayId: a.displayId } })}
                  >
                    Remove
                  </button>
                </div>
              ))}
            </div>
          </div>
          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>Unlocked Achievements</h3>
            {unlockedAchievements.length === 0 && <p>No unlocked achievements available to display.</p>}
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px', alignItems: 'end' }}>
              <label>
                Achievement
                <select
                  aria-label="Achievement to display"
                  value={achievementSlot}
                  onChange={(e) => setAchievementSlot(Number(e.target.value))}
                  style={inputStyle}
                >
                  {unlockedAchievements.map((a, i) => (
                    <option key={a.achievementId} value={i}>{a.title}</option>
                  ))}
                </select>
              </label>
              <button
                type="button"
                style={buttonStyle}
                disabled={unlockedAchievements.length === 0}
                onClick={() => {
                  const selected = unlockedAchievements[achievementSlot];
                  if (selected) {
                    emit({
                      type: 'display-achievement',
                      payload: { achievementId: selected.achievementId, slot: achievementSlot },
                    });
                  }
                }}
              >
                Display in Case
              </button>
            </div>
          </div>
        </div>
      )}

      {activeTab === 'guild' && (
        <div style={panelStyle}>
          <h3 style={{ marginTop: 0 }}>Guild Banner</h3>
          {guildBanner ? (
            <div style={{ marginBottom: '12px' }}>
              <strong style={{ fontSize: '1.2rem' }}>{guildBanner.guildName}</strong>
              <p style={{ color: '#94A3B8', margin: '4px 0' }}>{guildBanner.motto}</p>
              <small style={{ color: '#64748B' }}>Since {formatDate(guildBanner.installedAt)}</small>
            </div>
          ) : (
            <p>No guild banner displayed.</p>
          )}
          <label style={{ display: 'block', maxWidth: '420px' }}>
            Banner motto
            <input
              aria-label="Guild banner motto"
              value={motto}
              onChange={(e) => setMotto(e.target.value)}
              style={inputStyle}
              placeholder="Your guild motto"
            />
          </label>
          <div style={{ display: 'flex', gap: '8px', marginTop: '12px' }}>
            <button type="button" style={buttonStyle} onClick={() => emit({ type: 'set-guild-banner', payload: { motto } })}>
              Set Banner
            </button>
            <button type="button" style={secondaryButtonStyle} onClick={() => emit({ type: 'clear-guild-banner' })}>
              Clear Banner
            </button>
          </div>
        </div>
      )}

      {activeTab === 'storage' && (
        <div style={panelStyle}>
          <h3 style={{ marginTop: 0 }}>Storage Expansion</h3>
          <p style={{ color: '#94A3B8' }}>
            Level {storage?.level ?? 0} / {storage?.maxLevel ?? 5} — capacity {storage?.capacity ?? 100}
          </p>
          <progress value={storage?.level ?? 0} max={storage?.maxLevel ?? 5} style={{ width: '100%' }} />
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: '12px', margin: '12px 0' }}>
            <div><strong>{storage?.baseCapacity ?? 100}</strong><p style={{ margin: 0, color: '#94A3B8' }}>Base capacity</p></div>
            <div><strong>+{storage?.bonusCapacity ?? 0}</strong><p style={{ margin: 0, color: '#94A3B8' }}>Expansion bonus</p></div>
          </div>
          <button
            type="button"
            style={buttonStyle}
            disabled={(storage?.level ?? 0) >= (storage?.maxLevel ?? 5)}
            onClick={() => emit({ type: 'expand-storage' })}
          >
            Expand Storage{storage?.nextCost ? ` (${storage.nextCost} Stellar Dust)` : ''}
          </button>
        </div>
      )}

      {activeTab === 'visitors' && (
        <div style={{ display: 'grid', gap: '12px' }}>
          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>Visit a House</h3>
            <div style={{ display: 'flex', gap: '8px', alignItems: 'end' }}>
              <label style={{ flex: 1 }}>
                House owner address
                <input
                  aria-label="House owner address"
                  value={visitorAddress}
                  onChange={(e) => setVisitorAddress(e.target.value)}
                  style={inputStyle}
                />
              </label>
              <button
                type="button"
                style={buttonStyle}
                disabled={!visitorAddress}
                onClick={() => emit({ type: 'visit', payload: { houseOwner: visitorAddress } })}
              >
                Visit
              </button>
            </div>
          </div>
          <div style={panelStyle}>
            <h3 style={{ marginTop: 0 }}>Recent Visitors ({visitors.length})</h3>
            {visitors.length === 0 && <p>No visitors recorded.</p>}
            <div style={{ display: 'grid', gap: '6px' }}>
              {visitors.map((v, i) => (
                <div key={`${v.visitor}-${i}`} style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <span>{shortAddress(v.visitor)}</span>
                  <span style={{ color: '#94A3B8' }}>{formatDate(v.visitedAt)}</span>
                  <span>{v.leftRating ? 'Rated ★' : 'No rating'}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </section>
  );
};

export default Housing;

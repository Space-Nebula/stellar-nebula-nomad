import React, { useState } from 'react';
import { Card, Button, spacing, colors } from '../design-system';

export interface SectorAnomaly {
  id: string;
  name: string;
  x: number;
  y: number;
  dangerLevel: 'LOW' | 'MEDIUM' | 'HIGH';
  resourceType: string;
}

export interface ExplorationProps {
  initialSector?: string;
  anomalies?: SectorAnomaly[];
  fuelLevel?: number; // 0 - 100
  onScanSector?: (sectorName: string) => void;
  onNavigate?: (x: number, y: number) => void;
}

export const Exploration: React.FC<ExplorationProps> = ({
  initialSector = 'Sector 4-Zeta',
  anomalies = [
    { id: 'an-1', name: 'Gravitational Rift Alpha', x: 25, y: 40, dangerLevel: 'HIGH', resourceType: 'Dark Matter' },
    { id: 'an-2', name: 'Crystalline Asteroid Cluster', x: 65, y: 20, dangerLevel: 'LOW', resourceType: 'Silica & Quartz' },
    { id: 'an-3', name: 'Derelict Cargo Pod', x: 50, y: 75, dangerLevel: 'MEDIUM', resourceType: 'Scrap Tech' },
  ],
  fuelLevel = 82,
  onScanSector,
  onNavigate,
}) => {
  const [selectedAnomaly, setSelectedAnomaly] = useState<SectorAnomaly | null>(null);
  const [isScanning, setIsScanning] = useState<boolean>(false);

  const handleScan = () => {
    setIsScanning(true);
    if (onScanSector) onScanSector(initialSector);
    setTimeout(() => {
      setIsScanning(false);
    }, 1200);
  };

  return (
    <div style={{ padding: spacing.lg, maxWidth: '1200px', margin: '0 auto' }}>
      <header style={{ marginBottom: spacing.lg, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h1 style={{ margin: 0, fontSize: '1.75rem', color: colors.text }}>Nebula Exploration Interface</h1>
          <p style={{ margin: `${spacing.xs} 0 0 0`, color: colors.textMuted }}>Current Location: {initialSector}</p>
        </div>
        <div style={{ display: 'flex', gap: spacing.md, alignItems: 'center' }}>
          <div style={{ fontSize: '0.875rem', color: colors.textMuted }}>
            Ship Fuel: <span style={{ color: colors.warning, fontWeight: 'bold' }}>{fuelLevel}%</span>
          </div>
          <Button variant="primary" onClick={handleScan} disabled={isScanning}>
            {isScanning ? 'Scanning Sector…' : 'Scan Sector'}
          </Button>
        </div>
      </header>

      <div style={{ display: 'grid', gridTemplateColumns: '2fr 1fr', gap: spacing.lg }}>
        {/* Interactive Nebula Map Canvas / Matrix */}
        <Card style={{ position: 'relative', height: '450px', background: 'radial-gradient(ellipse at center, #1E1B4B 0%, #0B0F19 100%)', overflow: 'hidden' }}>
          <div style={{ position: 'absolute', top: spacing.sm, left: spacing.sm, fontSize: '0.8rem', color: colors.textMuted }}>
            Navigation Radar Matrix (Click anomaly to lock coordinates)
          </div>

          {/* Anomaly Markers */}
          {anomalies.map((an) => {
            const isSelected = selectedAnomaly?.id === an.id;
            return (
              <button
                key={an.id}
                onClick={() => setSelectedAnomaly(an)}
                style={{
                  position: 'absolute',
                  left: `${an.x}%`,
                  top: `${an.y}%`,
                  transform: 'translate(-50%, -50%)',
                  background: isSelected ? colors.primary : an.dangerLevel === 'HIGH' ? colors.danger : colors.secondary,
                  border: isSelected ? `2px solid #FFFFFF` : `1px solid ${colors.border}`,
                  borderRadius: '50%',
                  width: '28px',
                  height: '28px',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  cursor: 'pointer',
                  boxShadow: `0 0 12px ${isSelected ? colors.primary : 'rgba(0,0,0,0.5)'}`,
                }}
                title={an.name}
              >
                <span style={{ fontSize: '10px', color: '#000000', fontWeight: 'bold' }}>!</span>
              </button>
            );
          })}
        </Card>

        {/* Anomaly Inspection & Navigation Controls */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: spacing.md }}>
          <Card>
            <h2 style={{ fontSize: '1.125rem', marginTop: 0, marginBottom: spacing.md, color: colors.text }}>Anomaly Telemetry</h2>
            {selectedAnomaly ? (
              <div>
                <h3 style={{ fontSize: '1rem', color: colors.primary, margin: 0 }}>{selectedAnomaly.name}</h3>
                <dl style={{ margin: `${spacing.sm} 0`, fontSize: '0.875rem' }}>
                  <dt style={{ color: colors.textMuted }}>Resource Signature:</dt>
                  <dd style={{ margin: 0, fontWeight: 500, color: colors.success }}>{selectedAnomaly.resourceType}</dd>
                  <dt style={{ color: colors.textMuted, marginTop: spacing.xs }}>Danger Classification:</dt>
                  <dd style={{ margin: 0, fontWeight: 500, color: selectedAnomaly.dangerLevel === 'HIGH' ? colors.danger : colors.warning }}>
                    {selectedAnomaly.dangerLevel}
                  </dd>
                  <dt style={{ color: colors.textMuted, marginTop: spacing.xs }}>Relative Coordinates:</dt>
                  <dd style={{ margin: 0 }}>X: {selectedAnomaly.x} | Y: {selectedAnomaly.y}</dd>
                </dl>
                <Button
                  variant="secondary"
                  style={{ width: '100%', marginTop: spacing.sm }}
                  onClick={() => onNavigate && onNavigate(selectedAnomaly.x, selectedAnomaly.y)}
                >
                  Engage Thrusters to Coordinates
                </Button>
              </div>
            ) : (
              <p style={{ color: colors.textMuted, fontSize: '0.9rem' }}>
                Select an anomaly signature on the radar to inspect telemetry and plot course.
              </p>
            )}
          </Card>
        </div>
      </div>
    </div>
  );
};

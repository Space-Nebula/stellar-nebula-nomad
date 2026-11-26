import React from 'react';
import { PortalLink, adjacency, travelBlocker } from '../utils/portals';

export interface PortalNetworkProps {
  /** Pages from `get_portal_topology` merged with `get_portal_metadata`. */
  links: PortalLink[];
  currentNebula: number;
  traveler: string;
  /** Portal IDs whose allowlist contains the traveler. */
  allowlisted?: number[];
  onTravel: (portalId: number) => void;
  /** Owner / admin action; omitted for other players. */
  onToggleOpen?: (portalId: number, open: boolean) => void;
}

/** Portal discovery from the current nebula and the whole network topology (#711). */
export const PortalNetwork: React.FC<PortalNetworkProps> = ({
  links,
  currentNebula,
  traveler,
  allowlisted = [],
  onTravel,
  onToggleOpen,
}) => {
  const outgoing = adjacency(links).get(currentNebula) ?? [];

  return (
    <section aria-label="Portal network" style={{ padding: 16, borderRadius: 8, background: '#0F172A', color: '#F8FAFC' }}>
      <h2 style={{ margin: 0, fontSize: '1.1rem' }}>Portals from nebula {currentNebula}</h2>
      {outgoing.length === 0 ? (
        <p>No portals discovered from this nebula.</p>
      ) : (
        <ul style={{ listStyle: 'none', padding: 0 }}>
          {outgoing.map((l) => {
            const blocker = travelBlocker(l, traveler, allowlisted.includes(l.id));
            return (
              <li key={l.id} data-testid={`portal-${l.id}`} style={{ marginBottom: 6 }}>
                {l.name ?? `Portal #${l.id}`} → nebula {l.targetNebula} ·{' '}
                <meter min={0} max={100} value={l.stability} aria-label={`portal ${l.id} stability`} /> {l.stability}% ·{' '}
                {l.travelCost} · {l.access}
                {l.tags && l.tags.length > 0 && <span> · {l.tags.join(', ')}</span>}{' '}
                <button type="button" disabled={blocker !== null} title={blocker ?? 'Travel'} onClick={() => onTravel(l.id)}>
                  Travel
                </button>
                {blocker && <span> ({blocker})</span>}
              </li>
            );
          })}
        </ul>
      )}

      <h3 style={{ fontSize: '0.95rem' }}>Network topology</h3>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.85rem' }}>
        <thead>
          <tr>
            <th align="left">Portal</th>
            <th align="left">Route</th>
            <th align="left">Stability</th>
            <th align="left">State</th>
            <th align="left">Access</th>
            {onToggleOpen && <th />}
          </tr>
        </thead>
        <tbody>
          {links.map((l) => (
            <tr key={l.id} data-testid={`link-${l.id}`}>
              <td>{l.name ?? `#${l.id}`}</td>
              <td>
                {l.sourceNebula} → {l.targetNebula}
              </td>
              <td>{l.stability}%</td>
              <td>{l.open ? 'Open' : 'Closed'}</td>
              <td>{l.access}</td>
              {onToggleOpen && (
                <td>
                  {l.owner === traveler && (
                    <button type="button" onClick={() => onToggleOpen(l.id, !l.open)}>
                      {l.open ? 'Close' : 'Reopen'}
                    </button>
                  )}
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
};

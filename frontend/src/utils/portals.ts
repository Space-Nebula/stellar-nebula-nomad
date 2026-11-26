/**
 * Client-side mirror of the on-chain portal rules (#711). Keep in sync with
 * src/portal_registry.rs.
 */

export const MIN_STABLE_PCT = 10;
export const DECAY_RATE_PER_DAY = 2;
export const BASE_TRAVEL_COST = 100;
export const SECONDS_PER_DAY = 86_400;

export type PortalAccessMode = 'Public' | 'OwnerOnly' | 'Allowlist';

/** Mirrors `PortalLink` from `get_portal_topology`, plus optional metadata. */
export interface PortalLink {
  id: number;
  sourceNebula: number;
  targetNebula: number;
  stability: number;
  travelCost: number;
  open: boolean;
  access: PortalAccessMode;
  owner: string;
  name?: string;
  tags?: string[];
}

/** Mirrors `current_stability`. */
export function currentStability(initialStability: number, lastRefreshed: number, now: number): number {
  const days = Math.floor(Math.max(0, now - lastRefreshed) / SECONDS_PER_DAY);
  return Math.max(0, initialStability - DECAY_RATE_PER_DAY * days);
}

/** Mirrors `travel_cost`: BASE * 100 / stability, infinite at 0. */
export function travelCost(stability: number): number {
  return stability === 0 ? Infinity : Math.floor((BASE_TRAVEL_COST * 100) / stability);
}

/** Why travel is refused, or null when it is allowed (mirrors `travel_through_portal_as`). */
export function travelBlocker(link: PortalLink, traveler: string, allowlisted: boolean): string | null {
  if (link.access === 'OwnerOnly' && link.owner !== traveler) return 'Owner only';
  if (link.access === 'Allowlist' && link.owner !== traveler && !allowlisted) return 'Not on allowlist';
  if (!link.open) return 'Closed';
  if (link.stability < MIN_STABLE_PCT) return 'Unstable';
  return null;
}

/** Outgoing portals per nebula, mirroring `discover_portals`. */
export function adjacency(links: PortalLink[]): Map<number, PortalLink[]> {
  const out = new Map<number, PortalLink[]>();
  for (const l of links) out.set(l.sourceNebula, [...(out.get(l.sourceNebula) ?? []), l]);
  return out;
}

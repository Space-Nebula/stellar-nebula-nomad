/**
 * Client-side mirror of the on-chain alliance rules (#712). Keep in sync with
 * src/alliance_manager.rs.
 */

export const MAX_MEMBERS_PER_ALLIANCE = 50;
export const MAX_MEMBER_CAP = 100;
export const TERRITORY_CLAIM_COST = 500;

export type AllianceRole = 'Member' | 'Officer' | 'Leader';
export type DiplomaticStatus = 'Allied' | 'Neutral' | 'AtWar';

export interface AllianceMemberView {
  address: string;
  role: AllianceRole;
  contribution: number;
}

export interface AllianceObjective {
  objectiveId: number;
  kind: string;
  target: number;
  progress: number;
  rewardXp: number;
  deadline: number;
  completed: boolean;
}

export interface AllianceRelationView {
  allianceId: number;
  name: string;
  status: DiplomaticStatus;
  /** Status the other leader proposed to us, if any. */
  incoming?: DiplomaticStatus;
}

export interface AllianceBenefits {
  level: number;
  memberCap: number;
  territoryDiscountBps: number;
}

const RANK: Record<AllianceRole, number> = { Member: 0, Officer: 1, Leader: 2 };
const HOSTILITY: Record<DiplomaticStatus, number> = { Allied: 0, Neutral: 1, AtWar: 2 };

/** Mirrors `get_alliance_benefits`. */
export function benefitsForLevel(level: number): AllianceBenefits {
  const bonus = Math.max(1, level) - 1;
  return {
    level: Math.max(1, level),
    memberCap: Math.min(MAX_MEMBERS_PER_ALLIANCE + bonus * 5, MAX_MEMBER_CAP),
    territoryDiscountBps: Math.min(bonus * 250, 2_500),
  };
}

/** Mirrors the discounted cost in `claim_guild_territory`. */
export function territoryCost(level: number): number {
  return Math.floor((TERRITORY_CLAIM_COST * (10_000 - benefitsForLevel(level).territoryDiscountBps)) / 10_000);
}

/** Mirrors `kick_member`: officers and up, target strictly lower rank. */
export function canKick(actor: AllianceRole, target: AllianceRole): boolean {
  return RANK[actor] >= RANK.Officer && RANK[target] < RANK[actor];
}

/** Mirrors `propose_relation`: hostile moves apply at once, friendly ones need the other leader. */
export function needsAgreement(current: DiplomaticStatus, next: DiplomaticStatus): boolean {
  return HOSTILITY[next] < HOSTILITY[current];
}

export function objectivePct(o: AllianceObjective): number {
  return Math.floor((o.progress * 100) / o.target);
}

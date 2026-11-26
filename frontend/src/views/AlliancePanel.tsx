import React from 'react';
import {
  AllianceMemberView,
  AllianceObjective,
  AllianceRelationView,
  AllianceRole,
  DiplomaticStatus,
  benefitsForLevel,
  canKick,
  needsAgreement,
  objectivePct,
  territoryCost,
} from '../utils/alliances';

export interface AlliancePanelProps {
  name: string;
  level: number;
  viewer: string;
  members: AllianceMemberView[];
  relations: AllianceRelationView[];
  objectives: AllianceObjective[];
  /** Ledger time in seconds. */
  now: number;
  onSetRole: (member: string, role: Exclude<AllianceRole, 'Leader'>) => void;
  onKick: (member: string) => void;
  onTransferLeadership: (member: string) => void;
  onProposeRelation: (allianceId: number, status: DiplomaticStatus) => void;
  onContribute: (objectiveId: number) => void;
}

const STATUS_TEXT: Record<DiplomaticStatus, string> = { Allied: 'Allied', Neutral: 'Neutral', AtWar: 'At war' };
const STATUSES: DiplomaticStatus[] = ['Allied', 'Neutral', 'AtWar'];

/** Alliance members and roles, diplomacy, shared objectives and level benefits (#712). */
export const AlliancePanel: React.FC<AlliancePanelProps> = ({
  name,
  level,
  viewer,
  members,
  relations,
  objectives,
  now,
  onSetRole,
  onKick,
  onTransferLeadership,
  onProposeRelation,
  onContribute,
}) => {
  const me = members.find((m) => m.address === viewer);
  const myRole: AllianceRole | undefined = me?.role;
  const isLeader = myRole === 'Leader';
  const benefits = benefitsForLevel(level);

  return (
    <section aria-label={`Alliance ${name}`} style={{ padding: 16, borderRadius: 8, background: '#0F172A', color: '#F8FAFC' }}>
      <h2 style={{ margin: 0, fontSize: '1.1rem' }}>
        {name} · level {benefits.level}
      </h2>
      <p data-testid="benefits">
        Members {members.length}/{benefits.memberCap} · territory claims cost {territoryCost(level)} (
        {benefits.territoryDiscountBps / 100}% off)
      </p>

      <h3 style={{ fontSize: '0.95rem' }}>Members</h3>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.85rem' }}>
        <thead>
          <tr>
            <th align="left">Member</th>
            <th align="left">Role</th>
            <th align="left">Contribution</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {members.map((m) => (
            <tr key={m.address} data-testid={`member-${m.address}`}>
              <td>{m.address}</td>
              <td>{m.role}</td>
              <td>{m.contribution}</td>
              <td>
                {isLeader && m.role !== 'Leader' && (
                  <>
                    <button type="button" onClick={() => onSetRole(m.address, m.role === 'Officer' ? 'Member' : 'Officer')}>
                      {m.role === 'Officer' ? 'Demote' : 'Promote'}
                    </button>
                    <button type="button" onClick={() => onTransferLeadership(m.address)}>
                      Make leader
                    </button>
                  </>
                )}
                {myRole && canKick(myRole, m.role) && (
                  <button type="button" onClick={() => onKick(m.address)}>
                    Kick
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      <h3 style={{ fontSize: '0.95rem' }}>Diplomacy</h3>
      {relations.length === 0 ? (
        <p>No other alliances known.</p>
      ) : (
        <ul style={{ listStyle: 'none', padding: 0 }}>
          {relations.map((r) => (
            <li key={r.allianceId} data-testid={`relation-${r.allianceId}`}>
              {r.name}: {STATUS_TEXT[r.status]}
              {r.incoming && <span> · they propose {STATUS_TEXT[r.incoming]}</span>}
              {isLeader &&
                STATUSES.filter((s) => s !== r.status).map((s) => (
                  <button key={s} type="button" onClick={() => onProposeRelation(r.allianceId, s)} style={{ marginInlineStart: 6 }}>
                    {r.incoming === s ? `Accept ${STATUS_TEXT[s]}` : needsAgreement(r.status, s) ? `Propose ${STATUS_TEXT[s]}` : `Declare ${STATUS_TEXT[s]}`}
                  </button>
                ))}
            </li>
          ))}
        </ul>
      )}

      <h3 style={{ fontSize: '0.95rem' }}>Objectives</h3>
      {objectives.length === 0 ? (
        <p>No shared objectives.</p>
      ) : (
        <ul style={{ listStyle: 'none', padding: 0 }}>
          {objectives.map((o) => {
            const closed = o.completed || now > o.deadline;
            return (
              <li key={o.objectiveId} data-testid={`objective-${o.objectiveId}`}>
                {o.kind}{' '}
                <progress max={o.target} value={o.progress} aria-label={`${o.kind} progress`} /> {o.progress}/{o.target} (
                {objectivePct(o)}%) · {o.rewardXp} XP · {o.completed ? 'Completed' : now > o.deadline ? 'Expired' : 'Open'}{' '}
                <button type="button" disabled={closed || !me} onClick={() => onContribute(o.objectiveId)}>
                  Contribute
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
};

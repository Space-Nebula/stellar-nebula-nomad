import React, { useState } from 'react';

export interface EntangledMessageView {
  pairId: number;
  senderShip: number;
  payloadHex: string;
  sentAt: number;
  sequence: number;
}

export interface CommsPanelProps {
  pairId: number;
  remainingUses: number;
  encryptionKeyHash?: string;
  messages: EntangledMessageView[];
  onSendMessage?: (payloadHex: string) => void;
  onSendGuildMessage?: (guildId: number, regionId: number, payloadHex: string) => void;
}

export const CommsPanel: React.FC<CommsPanelProps> = ({
  pairId,
  remainingUses,
  encryptionKeyHash,
  messages,
  onSendMessage,
  onSendGuildMessage,
}) => {
  const [draft, setDraft] = useState('');
  const [guildId, setGuildId] = useState(1);
  const [regionId, setRegionId] = useState(1);

  return (
    <section aria-label="Quantum Entanglement Communications" className="comms-panel">
      <header>
        <h2>Quantum Entanglement Pair #{pairId}</h2>
        <span>Remaining uses: {remainingUses}</span>
        {encryptionKeyHash && (
          <span>Key commitment: {encryptionKeyHash.slice(0, 12)}…</span>
        )}
      </header>

      <ul aria-label="Message history">
        {messages.map(msg => (
          <li key={`${msg.pairId}-${msg.sequence}`}>
            <strong>Ship #{msg.senderShip}</strong> (#{msg.sequence}): {msg.payloadHex}
          </li>
        ))}
      </ul>

      <form
        onSubmit={e => {
          e.preventDefault();
          if (draft.trim() && onSendMessage) {
            onSendMessage(draft.trim());
            setDraft('');
          }
        }}
      >
        <input
          type="text"
          aria-label="Encrypted payload"
          value={draft}
          onChange={e => setDraft(e.target.value)}
          placeholder="Enter encrypted 64-byte payload..."
        />
        <button type="submit" disabled={remainingUses <= 0}>
          Send Entangled Message
        </button>
        {onSendGuildMessage && (
          <button
            type="button"
            onClick={() => {
              if (draft.trim()) {
                onSendGuildMessage(guildId, regionId, draft.trim());
                setDraft('');
              }
            }}
          >
            Broadcast Guild (Region {regionId})
          </button>
        )}
      </form>
      <div>
        <label>
          Guild ID
          <input
            type="number"
            value={guildId}
            onChange={e => setGuildId(Number(e.target.value))}
          />
        </label>
        <label>
          Region ID
          <input
            type="number"
            value={regionId}
            onChange={e => setRegionId(Number(e.target.value))}
          />
        </label>
      </div>
    </section>
  );
};

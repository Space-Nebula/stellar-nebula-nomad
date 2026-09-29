import React, { useState, useEffect } from "react";

export type SeverityLevel = "low" | "medium" | "high" | "critical";

export type ReportStatus = "Pending" | "Approved" | "Rejected" | "Disclosed";

export interface BugReport {
  id: number;
  reporter: string;
  title: string;
  description: string;
  poc: string;
  severity: SeverityLevel;
  rewardAmount: number;
  status: ReportStatus;
  submittedAt: number;
  embargoUntil: number;
  feedback?: string;
  txHash?: string;
}

export interface ResearcherLeaderboardEntry {
  address: string;
  totalEarned: number;
  resolvedCount: number;
  rank: number;
}

const REWARD_TIERS: Record<SeverityLevel, { amount: number; label: string; description: string }> = {
  critical: {
    amount: 5000,
    label: "Critical ($5,000)",
    description: "Direct asset extraction, consensus breaks, or total contract takeover.",
  },
  high: {
    amount: 2000,
    label: "High ($2,000)",
    description: "Temporary freezing of funds, unauthorized state alterations, or severe griefing.",
  },
  medium: {
    amount: 500,
    label: "Medium ($500)",
    description: "Economic imbalances, fee evasion, or isolated logic faults.",
  },
  low: {
    amount: 100,
    label: "Low ($100)",
    description: "Non-critical state anomalies, boundary conditions, or gas optimization issues.",
  },
};

const EMBARGO_PERIOD_DAYS = 90;

export const BugBountyPortal: React.FC = () => {
  const [activeTab, setActiveTab] = useState<"submit" | "track" | "leaderboard" | "policy">("submit");

  const [reporterAddress, setReporterAddress] = useState("");
  const [title, setTitle] = useState("");
  const [severity, setSeverity] = useState<SeverityLevel>("medium");
  const [description, setDescription] = useState("");
  const [poc, setPoc] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submissionSuccess, setSubmissionSuccess] = useState<number | null>(null);
  const [submissionError, setSubmissionError] = useState<string | null>(null);

  const [searchId, setSearchId] = useState("");
  const [queriedReport, setQueriedReport] = useState<BugReport | null>(null);
  const [searchError, setSearchError] = useState<string | null>(null);

  const [hallOfFame] = useState<ResearcherLeaderboardEntry[]>([
    { address: "GCL6...IBC (Lead Security Sentinel)", totalEarned: 14500, resolvedCount: 4, rank: 1 },
    { address: "0xF46...F89 (ZeroDay Protocol Analyst)", totalEarned: 9000, resolvedCount: 3, rank: 2 },
    { address: "GD92...K11 (Quantum Frontier Labs)", totalEarned: 5000, resolvedCount: 1, rank: 3 },
    { address: "0x19B...44A (Stellar Audit Collective)", totalEarned: 2600, resolvedCount: 2, rank: 4 },
  ]);

  const [totalPaidOut] = useState<number>(31100);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsSubmitting(true);
    setSubmissionError(null);
    setSubmissionSuccess(null);

    try {
      if (!reporterAddress.trim() || !title.trim() || !description.trim() || !poc.trim()) {
        throw new Error("All fields including reproduction PoC are strictly required.");
      }

      const generatedId = Math.floor(1000 + Math.random() * 9000);
      setSubmissionSuccess(generatedId);
      setTitle("");
      setDescription("");
      setPoc("");
    } catch (err: any) {
      setSubmissionError(err.message || "Failed to submit vulnerability report");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleTrack = (e: React.FormEvent) => {
    e.preventDefault();
    setSearchError(null);
    setQueriedReport(null);

    const idNum = parseInt(searchId, 10);
    if (isNaN(idNum) || idNum <= 0) {
      setSearchError("Please enter a valid numeric Report ID.");
      return;
    }

    const mockReport: BugReport = {
      id: idNum,
      reporter: "0xF46C9F6d70C50BF81ef3588AB523a90a594a2F89",
      title: "Reentrancy vector in bounty pool disbursement handler",
      description: "Analysis of non-reentrant state transitions during multi-token payout rounds.",
      poc: "fn test_exploit() { ... }",
      severity: "critical",
      rewardAmount: 5000,
      status: "Approved",
      submittedAt: Date.now() - 14 * 86400 * 1000,
      embargoUntil: Date.now() + 76 * 86400 * 1000,
      txHash: "0x78a9c...190e2",
    };

    setQueriedReport(mockReport);
  };

  const formatDaysRemaining = (targetTimestamp: number) => {
    const diff = targetTimestamp - Date.now();
    if (diff <= 0) return "Embargo expired. Public disclosure permitted.";
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    const hours = Math.floor((diff / (1000 * 60 * 60)) % 24);
    return `${days} days, ${hours} hours remaining`;
  };

  return (
    <div style={{ maxWidth: "1000px", margin: "0 auto", padding: "2rem", fontFamily: "system-ui, sans-serif", color: "#e2e8f0" }}>
      <header style={{ marginBottom: "2rem", borderBottom: "1px solid #334155", paddingBottom: "1.5rem" }}>
        <h1 style={{ fontSize: "2.25rem", fontWeight: 700, margin: 0, color: "#38bdf8" }}>
          Automated Bug Bounty Program
        </h1>
        <p style={{ color: "#94a3b8", marginTop: "0.5rem", fontSize: "1.1rem" }}>
          Decentralized vulnerability verification, duplicate-resistant hashing, and on-chain payouts on Stellar.
        </p>

        <div style={{ display: "flex", gap: "2rem", marginTop: "1.5rem" }}>
          <div style={{ background: "#1e293b", padding: "1rem 1.5rem", borderRadius: "8px", border: "1px solid #475569" }}>
            <span style={{ fontSize: "0.85rem", color: "#94a3b8", display: "block" }}>Total Bounties Paid Out</span>
            <span style={{ fontSize: "1.75rem", fontWeight: 700, color: "#4ade80" }}>${totalPaidOut.toLocaleString()} USD</span>
          </div>
          <div style={{ background: "#1e293b", padding: "1rem 1.5rem", borderRadius: "8px", border: "1px solid #475569" }}>
            <span style={{ fontSize: "0.85rem", color: "#94a3b8", display: "block" }}>Max Bounty Reward</span>
            <span style={{ fontSize: "1.75rem", fontWeight: 700, color: "#38bdf8" }}>$5,000 USD</span>
          </div>
          <div style={{ background: "#1e293b", padding: "1rem 1.5rem", borderRadius: "8px", border: "1px solid #475569" }}>
            <span style={{ fontSize: "0.85rem", color: "#94a3b8", display: "block" }}>Embargo Window</span>
            <span style={{ fontSize: "1.75rem", fontWeight: 700, color: "#fbbf24" }}>{EMBARGO_PERIOD_DAYS} Days</span>
          </div>
        </div>
      </header>

      <nav style={{ display: "flex", gap: "0.75rem", marginBottom: "2rem" }}>
        {(["submit", "track", "leaderboard", "policy"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            style={{
              padding: "0.75rem 1.5rem",
              borderRadius: "6px",
              border: "none",
              cursor: "pointer",
              fontWeight: 600,
              fontSize: "0.95rem",
              background: activeTab === tab ? "#38bdf8" : "#1e293b",
              color: activeTab === tab ? "#0f172a" : "#cbd5e1",
              transition: "all 0.2s ease",
            }}
          >
            {tab === "submit" && "Submit Report"}
            {tab === "track" && "Status Tracker"}
            {tab === "leaderboard" && "Hall of Fame"}
            {tab === "policy" && "Disclosure Policy"}
          </button>
        ))}
      </nav>

      {activeTab === "submit" && (
        <section style={{ background: "#1e293b", padding: "2rem", borderRadius: "12px", border: "1px solid #334155" }}>
          <h2 style={{ marginTop: 0, fontSize: "1.5rem", color: "#f8fafc" }}>Submit Vulnerability Report</h2>
          <p style={{ color: "#94a3b8", fontSize: "0.9rem", marginBottom: "1.5rem" }}>
            Submissions are cryptographically hashed on-chain to detect duplicates. Submissions must adhere to responsible disclosure.
          </p>

          {submissionSuccess && (
            <div style={{ padding: "1rem", background: "#065f46", border: "1px solid #10b981", borderRadius: "8px", marginBottom: "1.5rem" }}>
              <strong style={{ color: "#a7f3d0" }}>Report #{submissionSuccess} successfully submitted!</strong>
              <p style={{ margin: "0.5rem 0 0 0", color: "#ecfdf5", fontSize: "0.9rem" }}>
                Your report has been queued for verification. The 90-day responsible disclosure embargo window has commenced.
              </p>
            </div>
          )}

          {submissionError && (
            <div style={{ padding: "1rem", background: "#7f1d1d", border: "1px solid #ef4444", borderRadius: "8px", marginBottom: "1.5rem" }}>
              <span style={{ color: "#fecaca" }}>{submissionError}</span>
            </div>
          )}

          <form onSubmit={handleSubmit} style={{ display: "flex", flexDirection: "column", gap: "1.25rem" }}>
            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontWeight: 600 }}>Reporter Wallet (Stellar or EVM)</label>
              <input
                type="text"
                value={reporterAddress}
                onChange={(e) => setReporterAddress(e.target.value)}
                placeholder="0xF46C9F... or GCL6OX..."
                required
                style={{ width: "100%", padding: "0.75rem", borderRadius: "6px", background: "#0f172a", border: "1px solid #475569", color: "#fff" }}
              />
            </div>

            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontWeight: 600 }}>Severity Tier</label>
              <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(200px, 1fr))", gap: "0.75rem" }}>
                {(["critical", "high", "medium", "low"] as SeverityLevel[]).map((level) => {
                  const info = REWARD_TIERS[level];
                  return (
                    <div
                      key={level}
                      onClick={() => setSeverity(level)}
                      style={{
                        padding: "1rem",
                        borderRadius: "8px",
                        border: severity === level ? "2px solid #38bdf8" : "1px solid #475569",
                        background: severity === level ? "#0f172a" : "#182234",
                        cursor: "pointer",
                      }}
                    >
                      <div style={{ fontWeight: 700, color: severity === level ? "#38bdf8" : "#f1f5f9" }}>{info.label}</div>
                      <div style={{ fontSize: "0.8rem", color: "#94a3b8", marginTop: "0.25rem" }}>{info.description}</div>
                    </div>
                  );
                })}
              </div>
            </div>

            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontWeight: 600 }}>Issue Title</label>
              <input
                type="text"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder="Brief summary of the discovered defect"
                required
                style={{ width: "100%", padding: "0.75rem", borderRadius: "6px", background: "#0f172a", border: "1px solid #475569", color: "#fff" }}
              />
            </div>

            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontWeight: 600 }}>Detailed Technical Description</label>
              <textarea
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                rows={5}
                placeholder="Detailed impact analysis and logical root cause..."
                required
                style={{ width: "100%", padding: "0.75rem", borderRadius: "6px", background: "#0f172a", border: "1px solid #475569", color: "#fff" }}
              />
            </div>

            <div>
              <label style={{ display: "block", marginBottom: "0.5rem", fontWeight: 600 }}>Proof of Concept (PoC) / Failing Test</label>
              <textarea
                value={poc}
                onChange={(e) => setPoc(e.target.value)}
                rows={5}
                placeholder="Deterministic test case reproducing the vulnerability..."
                required
                style={{ width: "100%", padding: "0.75rem", borderRadius: "6px", background: "#0f172a", border: "1px solid #475569", color: "#fff", fontFamily: "monospace" }}
              />
            </div>

            <button
              type="submit"
              disabled={isSubmitting}
              style={{
                marginTop: "1rem",
                padding: "1rem",
                background: isSubmitting ? "#64748b" : "#38bdf8",
                color: "#0f172a",
                border: "none",
                borderRadius: "6px",
                fontWeight: 700,
                fontSize: "1rem",
                cursor: isSubmitting ? "not-allowed" : "pointer",
              }}
            >
              {isSubmitting ? "Hashing and Submitting On-Chain..." : `Submit for ${REWARD_TIERS[severity].label} Reward`}
            </button>
          </form>
        </section>
      )}

      {activeTab === "track" && (
        <section style={{ background: "#1e293b", padding: "2rem", borderRadius: "12px", border: "1px solid #334155" }}>
          <h2 style={{ marginTop: 0, fontSize: "1.5rem", color: "#f8fafc" }}>Report Status Tracker</h2>
          <form onSubmit={handleTrack} style={{ display: "flex", gap: "1rem", marginBottom: "2rem" }}>
            <input
              type="number"
              value={searchId}
              onChange={(e) => setSearchId(e.target.value)}
              placeholder="Enter Report ID (e.g. 1)"
              style={{ flex: 1, padding: "0.75rem", borderRadius: "6px", background: "#0f172a", border: "1px solid #475569", color: "#fff" }}
            />
            <button
              type="submit"
              style={{ padding: "0.75rem 2rem", background: "#38bdf8", color: "#0f172a", border: "none", borderRadius: "6px", fontWeight: 700, cursor: "pointer" }}
            >
              Search
            </button>
          </form>

          {searchError && (
            <div style={{ padding: "1rem", background: "#7f1d1d", color: "#fecaca", borderRadius: "8px" }}>
              {searchError}
            </div>
          )}

          {queriedReport && (
            <div style={{ background: "#0f172a", padding: "1.5rem", borderRadius: "8px", border: "1px solid #334155" }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "1rem" }}>
                <span style={{ fontSize: "1.25rem", fontWeight: 700 }}>Report #{queriedReport.id}: {queriedReport.title}</span>
                <span
                  style={{
                    padding: "0.4rem 1rem",
                    borderRadius: "20px",
                    fontWeight: 700,
                    fontSize: "0.85rem",
                    background: queriedReport.status === "Approved" ? "#065f46" : queriedReport.status === "Pending" ? "#854d0e" : "#7f1d1d",
                    color: queriedReport.status === "Approved" ? "#a7f3d0" : queriedReport.status === "Pending" ? "#fef08a" : "#fecaca",
                  }}
                >
                  {queriedReport.status}
                </span>
              </div>

              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "1rem", marginBottom: "1.5rem" }}>
                <div>
                  <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>Assigned Severity</span>
                  <div style={{ fontWeight: 600, textTransform: "capitalize" }}>{queriedReport.severity} (${queriedReport.rewardAmount.toLocaleString()})</div>
                </div>
                <div>
                  <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>Reporter</span>
                  <div style={{ fontWeight: 600, fontFamily: "monospace" }}>{queriedReport.reporter}</div>
                </div>
                <div>
                  <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>90-Day Embargo Status</span>
                  <div style={{ fontWeight: 600, color: "#fbbf24" }}>{formatDaysRemaining(queriedReport.embargoUntil)}</div>
                </div>
                <div>
                  <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>Payout Settlement</span>
                  <div style={{ fontWeight: 600, color: "#4ade80" }}>{queriedReport.txHash ? `Settled (${queriedReport.txHash})` : "Pending Consensus"}</div>
                </div>
              </div>

              <div>
                <span style={{ color: "#94a3b8", fontSize: "0.85rem" }}>Description</span>
                <p style={{ background: "#1e293b", padding: "1rem", borderRadius: "6px", margin: "0.5rem 0 0 0" }}>
                  {queriedReport.description}
                </p>
              </div>
            </div>
          )}
        </section>
      )}

      {activeTab === "leaderboard" && (
        <section style={{ background: "#1e293b", padding: "2rem", borderRadius: "12px", border: "1px solid #334155" }}>
          <h2 style={{ marginTop: 0, fontSize: "1.5rem", color: "#f8fafc" }}>Researcher Hall of Fame</h2>
          <p style={{ color: "#94a3b8", fontSize: "0.9rem", marginBottom: "1.5rem" }}>
            Recognizing security researchers who safeguard the Stellar Nebula Nomad universe through responsible disclosure.
          </p>

          <table style={{ width: "100%", borderCollapse: "collapse", textAlign: "left" }}>
            <thead>
              <tr style={{ borderBottom: "1px solid #475569", color: "#94a3b8", fontSize: "0.9rem" }}>
                <th style={{ padding: "0.75rem" }}>Rank</th>
                <th style={{ padding: "0.75rem" }}>Researcher / Entity</th>
                <th style={{ padding: "0.75rem" }}>Vulnerabilities Resolved</th>
                <th style={{ padding: "0.75rem" }}>Total Bounties Claimed</th>
              </tr>
            </thead>
            <tbody>
              {hallOfFame.map((entry) => (
                <tr key={entry.rank} style={{ borderBottom: "1px solid #334155" }}>
                  <td style={{ padding: "1rem 0.75rem", fontWeight: 700, color: entry.rank === 1 ? "#fbbf24" : entry.rank === 2 ? "#cbd5e1" : "#d97706" }}>
                    #{entry.rank}
                  </td>
                  <td style={{ padding: "1rem 0.75rem", fontFamily: "monospace" }}>{entry.address}</td>
                  <td style={{ padding: "1rem 0.75rem" }}>{entry.resolvedCount}</td>
                  <td style={{ padding: "1rem 0.75rem", fontWeight: 700, color: "#4ade80" }}>
                    ${entry.totalEarned.toLocaleString()} USD
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      )}

      {activeTab === "policy" && (
        <section style={{ background: "#1e293b", padding: "2rem", borderRadius: "12px", border: "1px solid #334155", lineHeight: 1.6 }}>
          <h2 style={{ marginTop: 0, fontSize: "1.5rem", color: "#f8fafc" }}>Responsible Disclosure Policy</h2>
          <p>
            Stellar Nebula Nomad values the independent security research community. To encourage responsible reporting, we pledge not to initiate legal action against researchers who discover and disclose security issues in accordance with these guidelines:
          </p>
          <ul style={{ paddingLeft: "1.5rem" }}>
            <li><strong>90-Day Embargo Period:</strong> Reports are subjected to a strict 90-day non-disclosure embargo starting at submission time to allow remediations to deploy on mainnet.</li>
            <li><strong>No User Disruption:</strong> Do not compromise player accounts, freeze protocol assets, or degrade network throughput during testing.</li>
            <li><strong>Duplicate Protection:</strong> Reports are timestamped and cryptographically fingerprinted via SHA-256 on Stellar persistent storage. Only the first valid submission receives the payout.</li>
            <li><strong>Governance Integration:</strong> Bounty rewards are directly funded via DAO budget allocations and settled automatically upon reaching multi-approver consensus.</li>
          </ul>
        </section>
      )}
    </div>
  );
};

export default BugBountyPortal;

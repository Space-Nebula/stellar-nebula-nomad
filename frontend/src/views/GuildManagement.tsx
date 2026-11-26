import React, { useState } from 'react';
import './GuildManagement.css';

export interface GuildMember {
  id: string;
  name: string;
  rank: 'owner' | 'admin' | 'officer' | 'member';
  level: number;
  joinedAt: number;
  lastActive: number;
  contribution: number;
}

export interface GuildQuest {
  id: number;
  name: string;
  description: string;
  reward: number;
  progress: number;
  maxProgress: number;
  difficulty: 'easy' | 'medium' | 'hard' | 'legendary';
  participants: number;
  status: 'active' | 'completed' | 'failed';
}

export interface LeaderboardEntry {
  rank: number;
  name: string;
  score: number;
  level: number;
  guildName?: string;
}

export interface UserProfile {
  id: string;
  name: string;
  level: number;
  experience: number;
  achievements: number;
  followers: number;
  following: number;
  bio: string;
  joinedAt: number;
}

export interface Guild {
  id: string;
  name: string;
  owner: string;
  members: GuildMember[];
  level: number;
  experience: number;
  treasury: number;
  description: string;
  banner?: string;
  createdAt: number;
  quests: GuildQuest[];
}

export interface GuildManagementProps {
  guild?: Guild;
  currentUser: UserProfile;
  members: GuildMember[];
  leaderboard: LeaderboardEntry[];
  quests: GuildQuest[];
  onQuestComplete?: (questId: number) => void;
  onMemberPromote?: (memberId: string) => void;
  onMemberKick?: (memberId: string) => void;
  onCreateQuest?: (questData: any) => void;
}

export const GuildManagement: React.FC<GuildManagementProps> = ({
  guild,
  currentUser,
  members,
  leaderboard,
  quests,
  onQuestComplete,
  onMemberPromote,
  onMemberKick,
  onCreateQuest,
}) => {
  const [activeTab, setActiveTab] = useState<'overview' | 'members' | 'quests' | 'leaderboard' | 'profile'>('overview');
  const [selectedMember, setSelectedMember] = useState<GuildMember | null>(null);
  const [selectedProfile, setSelectedProfile] = useState<UserProfile | null>(null);

  const getRankColor = (rank: string) => {
    const colors: Record<string, string> = {
      owner: '#ff0070',
      admin: '#ff8000',
      officer: '#00d9ff',
      member: '#888888',
    };
    return colors[rank] || '#ffffff';
  };

  const getDifficultyColor = (difficulty: string) => {
    const colors: Record<string, string> = {
      easy: '#00ff88',
      medium: '#ffaa00',
      hard: '#ff4444',
      legendary: '#ff0070',
    };
    return colors[difficulty] || '#ffffff';
  };

  const OverviewTab = () => (
    <div className="guild-tab">
      <div className="guild-header-info">
        <div className="guild-title">
          <h1>{guild?.name || 'Guild Name'}</h1>
          <span className="guild-level">Level {guild?.level || 1}</span>
        </div>
        <div className="guild-stats">
          <div className="stat-box">
            <span className="stat-label">Members</span>
            <span className="stat-value">{members.length}</span>
          </div>
          <div className="stat-box">
            <span className="stat-label">Treasury</span>
            <span className="stat-value">{guild?.treasury.toLocaleString()}</span>
          </div>
          <div className="stat-box">
            <span className="stat-label">Experience</span>
            <span className="stat-value">{guild?.experience.toLocaleString()}</span>
          </div>
        </div>
      </div>

      <div className="guild-description">
        <h3>About Guild</h3>
        <p>{guild?.description || 'No description provided'}</p>
      </div>

      <div className="guild-benefits">
        <h3>Guild Benefits</h3>
        <div className="benefits-grid">
          <div className="benefit-card">
            <span className="benefit-icon">🎯</span>
            <span className="benefit-name">Shared Quests</span>
          </div>
          <div className="benefit-card">
            <span className="benefit-icon">💰</span>
            <span className="benefit-name">Treasury</span>
          </div>
          <div className="benefit-card">
            <span className="benefit-icon">🏆</span>
            <span className="benefit-name">Leaderboards</span>
          </div>
          <div className="benefit-card">
            <span className="benefit-icon">🤝</span>
            <span className="benefit-name">Cooperation</span>
          </div>
          <div className="benefit-card">
            <span className="benefit-icon">📊</span>
            <span className="benefit-name">Statistics</span>
          </div>
          <div className="benefit-card">
            <span className="benefit-icon">🎪</span>
            <span className="benefit-name">Events</span>
          </div>
        </div>
      </div>
    </div>
  );

  const MembersTab = () => (
    <div className="guild-tab">
      <div className="members-container">
        <h3>Guild Members ({members.length})</h3>
        <div className="members-list">
          {members.map(member => (
            <div
              key={member.id}
              className={`member-card ${selectedMember?.id === member.id ? 'selected' : ''}`}
              onClick={() => setSelectedMember(member)}
              style={{ borderLeftColor: getRankColor(member.rank) }}
            >
              <div className="member-info">
                <div className="member-name">{member.name}</div>
                <div className="member-level">Level {member.level}</div>
              </div>
              <div className="member-rank" style={{ color: getRankColor(member.rank) }}>
                {member.rank.toUpperCase()}
              </div>
              <div className="member-contribution">Contrib: {member.contribution}</div>
            </div>
          ))}
        </div>

        {selectedMember && (
          <div className="member-detail-panel">
            <div className="detail-header">
              <h3>{selectedMember.name}</h3>
              <button className="close-btn" onClick={() => setSelectedMember(null)}>✕</button>
            </div>
            <div className="detail-content">
              <div className="detail-row">
                <span>Rank:</span>
                <strong style={{ color: getRankColor(selectedMember.rank) }}>
                  {selectedMember.rank.toUpperCase()}
                </strong>
              </div>
              <div className="detail-row">
                <span>Level:</span>
                <strong>{selectedMember.level}</strong>
              </div>
              <div className="detail-row">
                <span>Contribution:</span>
                <strong>{selectedMember.contribution}</strong>
              </div>
              <div className="detail-row">
                <span>Joined:</span>
                <strong>{new Date(selectedMember.joinedAt).toLocaleDateString()}</strong>
              </div>
              <div className="detail-row">
                <span>Last Active:</span>
                <strong>{new Date(selectedMember.lastActive).toLocaleString()}</strong>
              </div>

              <div className="member-actions">
                <button className="action-btn" onClick={() => onMemberPromote?.(selectedMember.id)}>
                  Promote
                </button>
                <button className="action-btn danger" onClick={() => onMemberKick?.(selectedMember.id)}>
                  Kick
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );

  const QuestsTab = () => (
    <div className="guild-tab">
      <h3>Guild Quests</h3>
      <div className="quests-grid">
        {quests.map(quest => (
          <div key={quest.id} className="quest-card" style={{ borderLeftColor: getDifficultyColor(quest.difficulty) }}>
            <div className="quest-header">
              <h4>{quest.name}</h4>
              <span className="quest-status" style={{ color: getDifficultyColor(quest.difficulty) }}>
                {quest.difficulty.toUpperCase()}
              </span>
            </div>
            <p className="quest-description">{quest.description}</p>
            <div className="quest-progress">
              <div className="progress-bar">
                <div className="progress-fill" style={{ width: `${(quest.progress / quest.maxProgress) * 100}%` }}></div>
              </div>
              <span className="progress-text">{quest.progress} / {quest.maxProgress}</span>
            </div>
            <div className="quest-info">
              <span>Reward: {quest.reward}</span>
              <span>Participants: {quest.participants}</span>
            </div>
            {quest.status === 'active' && (
              <button className="complete-btn" onClick={() => onQuestComplete?.(quest.id)}>
                Complete Quest
              </button>
            )}
            {quest.status === 'completed' && (
              <div className="status-badge completed">✓ Completed</div>
            )}
          </div>
        ))}
      </div>
    </div>
  );

  const LeaderboardTab = () => (
    <div className="guild-tab">
      <h3>Guild Leaderboard</h3>
      <div className="leaderboard-container">
        <div className="leaderboard-list">
          {leaderboard.map(entry => (
            <div key={entry.rank} className="leaderboard-entry">
              <div className="rank-badge">
                {entry.rank === 1 && '🥇'}
                {entry.rank === 2 && '🥈'}
                {entry.rank === 3 && '🥉'}
                {entry.rank > 3 && `#${entry.rank}`}
              </div>
              <div className="entry-info">
                <div className="entry-name">{entry.name}</div>
                <div className="entry-guild">{entry.guildName && `${entry.guildName}`}</div>
              </div>
              <div className="entry-level">Lv {entry.level}</div>
              <div className="entry-score">{entry.score.toLocaleString()}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );

  const ProfileTab = () => (
    <div className="guild-tab">
      <div className="profile-container">
        <div className="profile-header">
          <h2>{currentUser.name}</h2>
          <span className="profile-level">Level {currentUser.level}</span>
        </div>

        <div className="profile-stats">
          <div className="profile-stat">
            <span className="stat-label">Experience</span>
            <span className="stat-value">{currentUser.experience.toLocaleString()}</span>
          </div>
          <div className="profile-stat">
            <span className="stat-label">Achievements</span>
            <span className="stat-value">{currentUser.achievements}</span>
          </div>
          <div className="profile-stat">
            <span className="stat-label">Followers</span>
            <span className="stat-value">{currentUser.followers}</span>
          </div>
          <div className="profile-stat">
            <span className="stat-label">Following</span>
            <span className="stat-value">{currentUser.following}</span>
          </div>
        </div>

        <div className="profile-bio">
          <h3>Bio</h3>
          <p>{currentUser.bio || 'No bio provided'}</p>
        </div>

        <div className="profile-actions">
          <button className="profile-btn">Edit Profile</button>
          <button className="profile-btn">Share Profile</button>
          <button className="profile-btn">View Achievements</button>
        </div>
      </div>
    </div>
  );

  return (
    <div className="guild-management">
      <div className="guild-container">
        <div className="tabs-navigation">
          <button
            className={`nav-tab ${activeTab === 'overview' ? 'active' : ''}`}
            onClick={() => setActiveTab('overview')}
          >
            Overview
          </button>
          <button
            className={`nav-tab ${activeTab === 'members' ? 'active' : ''}`}
            onClick={() => setActiveTab('members')}
          >
            Members
          </button>
          <button
            className={`nav-tab ${activeTab === 'quests' ? 'active' : ''}`}
            onClick={() => setActiveTab('quests')}
          >
            Quests
          </button>
          <button
            className={`nav-tab ${activeTab === 'leaderboard' ? 'active' : ''}`}
            onClick={() => setActiveTab('leaderboard')}
          >
            Leaderboard
          </button>
          <button
            className={`nav-tab ${activeTab === 'profile' ? 'active' : ''}`}
            onClick={() => setActiveTab('profile')}
          >
            Profile
          </button>
        </div>

        <div className="tab-content-area">
          {activeTab === 'overview' && <OverviewTab />}
          {activeTab === 'members' && <MembersTab />}
          {activeTab === 'quests' && <QuestsTab />}
          {activeTab === 'leaderboard' && <LeaderboardTab />}
          {activeTab === 'profile' && <ProfileTab />}
        </div>
      </div>
    </div>
  );
};

export default GuildManagement;

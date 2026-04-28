# Community Event Building Guide

## Introduction

The Event Scheduler is more than just a technical feature—it's a powerful tool for building and maintaining an engaged community in Nebula Nomad. This guide provides strategies, best practices, and templates for creating memorable community experiences.

## Community Engagement Strategy

### The Event Calendar Philosophy

A well-designed event calendar should:

- **Create Anticipation**: Regular events give players something to look forward to
- **Maintain Variety**: Different event types appeal to different player preferences
- **Build Routine**: Consistent scheduling helps players plan their participation
- **Foster Competition**: Leaderboards and rewards drive engagement
- **Encourage Cooperation**: Team events build community bonds

### Event Frequency Recommendations

| Event Type            | Recommended Frequency | Typical Reward Pool |
| --------------------- | --------------------- | ------------------- |
| Weekly Festival       | Every 7 days          | 50,000 - 100,000    |
| Raid Boss             | 2-3 times per week    | 25,000 - 50,000     |
| PvP Tournament        | Weekly                | 40,000 - 75,000     |
| Harvest Competition   | Daily                 | 10,000 - 20,000     |
| Exploration Challenge | 2-3 times per week    | 15,000 - 30,000     |

## Event Templates

### Template 1: Weekly Festival

**Purpose**: Major community gathering with large rewards

**Schedule**: Every Sunday at 8 PM UTC

**Reward Structure**:

- Total Pool: 100,000
- Top 10 players: 60% of pool
- Participation rewards: 40% of pool

**Implementation**:

```rust
// Schedule weekly festival
let event_id = schedule_weekly_festival(
    env,
    admin,
    100_000i128,
)?;
```

**Promotion Strategy**:

- Announce 7 days in advance
- Daily reminders starting 3 days before
- Social media countdown
- In-game notifications

### Template 2: Raid Marathon

**Purpose**: Cooperative boss battles

**Schedule**: Tuesday and Thursday at 7 PM UTC

**Reward Structure**:

- Total Pool: 40,000
- Damage dealers: 50%
- Support roles: 30%
- Participation: 20%

**Implementation**:

```rust
let current_time = env.ledger().timestamp();
let tuesday = current_time + (2 * 24 * 60 * 60);

let raid_id = schedule_event(
    env,
    admin,
    symbol_short!("raid"),
    tuesday,
    40_000i128,
)?;
```

### Template 3: PvP Championship

**Purpose**: Competitive player vs player tournament

**Schedule**: Monthly, first Saturday at 6 PM UTC

**Reward Structure**:

- 1st Place: 40%
- 2nd Place: 25%
- 3rd Place: 15%
- Top 10: 15%
- Participation: 5%

**Implementation**:

```rust
let championship_time = current_time + (30 * 24 * 60 * 60);

let pvp_id = schedule_event(
    env,
    admin,
    symbol_short!("pvp"),
    championship_time,
    200_000i128,
)?;
```

### Template 4: Daily Harvest Hour

**Purpose**: Quick engagement with daily rewards

**Schedule**: Every day at 12 PM UTC

**Reward Structure**:

- Top 20 harvesters: 80%
- All participants: 20%

**Implementation**:

```rust
let tomorrow_noon = current_time + (24 * 60 * 60);

let harvest_id = schedule_event(
    env,
    admin,
    symbol_short!("harvest"),
    tomorrow_noon,
    15_000i128,
)?;
```

### Template 5: Exploration Weekend

**Purpose**: Discovery and adventure

**Schedule**: Every other weekend

**Reward Structure**:

- Most nebulae discovered: 40%
- Rare discoveries: 30%
- Participation: 30%

**Implementation**:

```rust
let weekend_start = current_time + (7 * 24 * 60 * 60);

let explore_id = schedule_event(
    env,
    admin,
    symbol_short!("explore"),
    weekend_start,
    50_000i128,
)?;
```

## Seasonal Event Series

### Spring Cosmic Bloom (4 weeks)

**Theme**: Renewal and growth

**Week 1**: Opening Festival (200,000 rewards)
**Week 2**: Harvest Bonanza (150,000 rewards)
**Week 3**: Exploration Rush (175,000 rewards)
**Week 4**: Grand Finale Festival (500,000 rewards)

**Total Investment**: 1,025,000 rewards

### Summer Stellar Wars (4 weeks)

**Theme**: Competition and glory

**Week 1**: PvP Qualifiers (100,000 rewards)
**Week 2**: Raid Gauntlet (150,000 rewards)
**Week 3**: PvP Semi-Finals (200,000 rewards)
**Week 4**: Championship Finals (750,000 rewards)

**Total Investment**: 1,200,000 rewards

### Fall Harvest Moon (4 weeks)

**Theme**: Abundance and cooperation

**Week 1**: Harvest Festival (150,000 rewards)
**Week 2**: Cooperative Raids (200,000 rewards)
**Week 3**: Resource Trading Fair (175,000 rewards)
**Week 4**: Thanksgiving Feast (400,000 rewards)

**Total Investment**: 925,000 rewards

### Winter Nebula Nights (4 weeks)

**Theme**: Mystery and exploration

**Week 1**: Exploration Challenge (175,000 rewards)
**Week 2**: Mystery Raid (200,000 rewards)
**Week 3**: Holiday PvP (225,000 rewards)
**Week 4**: New Year Celebration (600,000 rewards)

**Total Investment**: 1,200,000 rewards

## Event Promotion Best Practices

### Pre-Event (7 days before)

1. **Initial Announcement**
   - Post on all social channels
   - In-game notification
   - Discord announcement
   - Email newsletter

2. **Event Details**
   - Clear start time (with timezone)
   - Reward structure
   - Participation requirements
   - Special rules or mechanics

3. **Build Anticipation**
   - Teaser content
   - Previous event highlights
   - Player testimonials

### Mid-Week (3-4 days before)

1. **Reminder Campaign**
   - Social media posts
   - In-game countdown
   - Discord reminders

2. **Strategy Content**
   - Tips for success
   - Team formation guides
   - Equipment recommendations

### Final Day (24 hours before)

1. **Final Countdown**
   - Hourly reminders
   - Live countdown timer
   - Last-minute registration

2. **Community Hype**
   - Player predictions
   - Team announcements
   - Streamer participation

### During Event

1. **Live Updates**
   - Real-time leaderboards
   - Milestone announcements
   - Community highlights

2. **Engagement**
   - Live commentary
   - Player spotlights
   - Social media interaction

### Post-Event

1. **Results**
   - Winner announcements
   - Final leaderboards
   - Reward distribution confirmation

2. **Highlights**
   - Best moments
   - Player achievements
   - Event statistics

3. **Feedback**
   - Community survey
   - Improvement suggestions
   - Next event teaser

## Reward Pool Management

### Budget Allocation

**Monthly Reward Budget**: 1,000,000

| Category          | Allocation | Amount  |
| ----------------- | ---------- | ------- |
| Weekly Festivals  | 40%        | 400,000 |
| Daily Events      | 25%        | 250,000 |
| Special Events    | 20%        | 200,000 |
| Seasonal Series   | 10%        | 100,000 |
| Emergency Reserve | 5%         | 50,000  |

### Dynamic Reward Scaling

Adjust rewards based on:

- **Participation**: Higher turnout = larger pools
- **Competition**: Closer matches = bonus rewards
- **Milestones**: Community achievements unlock bonuses
- **Sponsorships**: Partner contributions increase pools

### Sustainability

- Monitor reward distribution rates
- Track player retention metrics
- Adjust pools based on economy health
- Plan for long-term sustainability

## Player Retention Strategies

### Daily Engagement

- **Daily Harvest Hour**: Quick 15-minute event
- **Login Bonuses**: Rewards for checking event calendar
- **Streak Rewards**: Consecutive participation bonuses

### Weekly Commitment

- **Weekly Festival**: Major community gathering
- **Raid Schedule**: Consistent team events
- **PvP Nights**: Regular competitive opportunities

### Monthly Milestones

- **Championship Events**: Major tournaments
- **Seasonal Transitions**: Theme changes
- **Community Challenges**: Collective goals

### Long-Term Investment

- **Seasonal Series**: Multi-week narratives
- **Leaderboard Seasons**: Quarterly rankings
- **Achievement Systems**: Long-term goals

## Community Building Activities

### Team Formation

Encourage players to form teams for:

- Raid events
- Cooperative challenges
- Alliance competitions

**Benefits**:

- Stronger social bonds
- Higher retention
- Natural mentorship
- Community leadership

### Content Creation

Support community content:

- Event guides and strategies
- Highlight videos
- Streaming partnerships
- Fan art contests

**Rewards**:

- Creator recognition
- Exclusive rewards
- Featured content
- Community spotlight

### Player Governance

Future implementation of player voting:

- Event type selection
- Reward distribution
- Schedule preferences
- New event ideas

## Metrics and Analytics

### Key Performance Indicators

1. **Participation Rate**
   - Total players per event
   - Percentage of active users
   - Trend over time

2. **Engagement Duration**
   - Average time per event
   - Completion rates
   - Return participation

3. **Reward Distribution**
   - Total rewards claimed
   - Distribution fairness
   - Economic impact

4. **Community Growth**
   - New player acquisition
   - Retention rates
   - Social media engagement

### Success Metrics

| Metric             | Target    | Excellent  |
| ------------------ | --------- | ---------- |
| Participation Rate | >30%      | >50%       |
| Event Completion   | >70%      | >85%       |
| Return Rate        | >60%      | >80%       |
| Community Growth   | +5%/month | +10%/month |

## Crisis Management

### Event Cancellation

If an event must be cancelled:

1. Immediate notification to all players
2. Clear explanation of reason
3. Compensation plan
4. Rescheduling announcement

### Technical Issues

If technical problems occur:

1. Pause event if possible
2. Document all issues
3. Fair compensation for affected players
4. Post-mortem analysis

### Community Concerns

Address player feedback:

1. Active listening
2. Transparent communication
3. Fair resolution
4. Process improvements

## Future Enhancements

### Player-Voted Events

Allow community to vote on:

- Event types
- Reward pools
- Schedule times
- Special mechanics

**Implementation Plan**:

```rust
// Proposed voting system
pub fn propose_event(
    env: Env,
    proposer: Address,
    event_type: Symbol,
    start_time: u64,
) -> Result<u64, EventError>;

pub fn vote_on_proposal(
    env: Env,
    voter: Address,
    proposal_id: u64,
    support: bool,
) -> Result<(), EventError>;
```

### Dynamic Event Generation

AI-powered event creation based on:

- Player behavior patterns
- Community preferences
- Economic conditions
- Seasonal trends

### Cross-Game Events

Collaborate with other projects:

- Shared reward pools
- Cross-platform participation
- Unified leaderboards
- Ecosystem growth

## Conclusion

The Event Scheduler is a foundation for building a thriving community. Success comes from:

1. **Consistency**: Regular, predictable events
2. **Variety**: Different types for different players
3. **Fairness**: Balanced rewards and opportunities
4. **Communication**: Clear, timely information
5. **Adaptation**: Responsive to community feedback

By following these guidelines and continuously iterating based on player feedback, you can create a vibrant, engaged community that grows and thrives over time.

## Resources

- Event Scheduler API Documentation: `docs/EVENT_SCHEDULER_GUIDE.md`
- Example Implementations: `examples/event_scheduler_example.rs`
- Test Suite: `tests/test_event_scheduler.rs`

## Support

For questions or suggestions:

- GitHub Issues: Report bugs or request features
- Discord: Join community discussions
- Documentation: Comprehensive guides and examples

# Revenue Attribution and Analytics System

## Overview

The Revenue Attribution and Analytics System (`src/revenue_attribution.rs`) provides comprehensive on-chain tracking of acquisition channels, conversion funnels, multi-touch attribution models, return on investment (ROI/ROAS), customer acquisition cost (CAC), and macro-economic impact across the Stellar Nebula Nomad game economy.

## Core Capabilities

### 1. Revenue and Spend Tracking
- **Multi-channel attribution**: Attribute ecosystem revenue to acquisition channels (e.g., `organic`, `referral`, `ad_campaign`, `guild_partner`).
- **Spend tracking**: Record acquisition expenses on a per-channel basis to calculate exact financial efficiency.
- **Categorized economic sources**: Track revenues by category:
  - `MarketplaceSale`
  - `InGamePurchase`
  - `Subscription`
  - `TournamentFee`
  - `StakingYieldFee`
  - `CraftingFee`
  - `Direct`
  - `Referral`
  - `EscrowFee`
  - `BridgeFee`

### 2. Multi-Touch Attribution Models
Supports multiple industry-standard attribution models to distribute credit for player conversion across touchpoints:
- **First-Touch (`FirstTouch`)**: 100% credit to the discovery channel.
- **Last-Touch (`LastTouch`)**: 100% credit to the final channel before conversion.
- **Linear (`Linear`)**: Uniform credit split across all recorded touchpoints.
- **Time-Decay (`TimeDecay`)**: Progressively higher weights assigned to recent touchpoints.
- **Position-Based (`PositionBased`)**: U-shaped distribution allocating 40% to first touch, 40% to last touch, and 20% shared evenly across intermediate touches.

### 3. Conversion Funnel Tracking
Tracks granular progression across acquisition stages:
- `Impression`
- `Click`
- `Signup`
- `Purchase`

Computes Click-Through Rate (CTR) and Conversion Rate (CVR) in basis points (`1/10,000`).

### 4. Unit Economics and ROI Analysis
- **ROI (Return on Investment)**: `(Revenue - Spend) / Spend * 10,000 bps`
- **ROAS (Return on Ad Spend)**: `Revenue / Spend * 10,000 bps`
- **CAC (Customer Acquisition Cost)**: `Spend / Conversions`
- **Net Ecosystem Profit**: Net token flow retained by ecosystem.

### 5. Macro-Economic Impact & Dashboard
- Aggregate gross revenue and acquisition spend.
- Velocity ratio and active channel distribution.
- Top-performing acquisition channel discovery.
- Comprehensive `RevenueAnalyticsDashboard` snapshot for operators.

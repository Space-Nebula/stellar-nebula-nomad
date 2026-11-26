# Frontend Application Architecture

## Overview
The Stellar Nebula Nomad frontend application provides a modular, performant, and accessible user interface for interacting with the Stellar Nebula Nomad Soroban smart contracts.

## Architecture Stack

### 1. Framework & Core
- **Next.js / React 18+**: App router architecture, Server/Client components separation, and SSR/SSG capabilities.
- **TypeScript**: Strict type definitions covering contract types, API requests/responses, and component props.

### 2. State Management
- **Zustand / React Context**: Lightweight, decoupled stores for client state:
  - `useWalletStore`: Active account, network connection, balance, signature requests.
  - `useGameStore`: Sector map, ship telemetry, anomaly scanner status, active quests.
  - `useUIStore`: Modals, notifications, theme preferences, accessibility settings.

### 3. Smart Contract & API Client
- **Soroban Client Integration**: Type-safe contract wrappers connecting to RPC nodes via `@stellar/stellar-sdk` and `@stellarforge-protocol/sdk`.
- **API Services**: Structured services in `frontend/src/services/` for game analytics, metadata resolution, and event polling.

### 4. Component System & Styling
- **Atomic Design**: Reusable buttons, inputs, cards, dialogs, and HUD elements located in `frontend/src/components/`.
- **Storybook**: Interactive component documentation and visual testing (`.storybook/`).
- **A11y & Theming**: Integrated high-contrast and colorblind palettes in `frontend/src/a11y/` and `frontend/src/theme/`.

### 5. Routing & Layout System
- Declarative routing supporting main views:
  - `/`: Mission Control / Dashboard
  - `/explore`: Interactive 3D Star Map and Anomaly Scanner
  - `/hangar`: Ship NFTs, upgrades, and module customization
  - `/governance`: DAO proposals and voting
  - `/leaderboards`: Competitive rankings and achievements

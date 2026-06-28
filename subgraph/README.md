# Stellar Nebula Nomad Subgraph

Event indexer for the Stellar Nebula Nomad Soroban smart contract. Maps on-chain
events into a queryable GraphQL schema optimized for frontend consumption.

## Directory Structure

```
subgraph/
├── schema.graphql        # GraphQL entity definitions
├── subgraph.yaml         # Indexer manifest (data sources, event handlers)
├── src/
│   └── mappings.ts       # AssemblyScript event handler implementations
├── abis/
│   └── StellarNebulaNomad.json  # Contract ABI (generated, not committed)
├── package.json          # Build scripts & dependencies
└── README.md             # This file
```

## Setup

### 1. Install Dependencies

```bash
npm install -g @graphprotocol/graph-cli
cd subgraph
npm install
```

### 2. Generate Types from ABI

Place your contract ABI at `abis/StellarNebulaNomad.json`, then:

```bash
graph codegen
```

### 3. Build the Subgraph

```bash
graph build
```

### 4. Deploy

#### The Graph Hosted Service

```bash
graph create your-username/stellar-nebula-nomad --node https://api.thegraph.com/deploy/
graph deploy your-username/stellar-nebula-nomad \
  --ipfs https://api.thegraph.com/ipfs/ \
  --node https://api.thegraph.com/deploy/ \
  --version-label v1.0.0
```

#### Local Graph Node (Docker)

```bash
# Start a local Graph Node stack
git clone https://github.com/graphprotocol/graph-node
cd graph-node/docker
docker compose up

# Deploy to local node
graph deploy stellar-nebula-nomad \
  --ipfs http://localhost:5001 \
  --node http://localhost:8020 \
  --version-label v1.0.0-local
```

## Environment Variables

| Variable           | Description                              | Example                          |
|--------------------|------------------------------------------|----------------------------------|
| `CONTRACT_ADDRESS` | Deployed Soroban contract address         | `CDYZB...`                       |

Set these before building:

```bash
export CONTRACT_ADDRESS="CDYZB3RHDH6H..."
```

## Indexed Entities

- **Player** — on-chain player profiles, scan counts, essence earnings
- **Ship** — NFT ship assets with metadata, ownership, progression
- **NebulaScan** — nebula generation results, rarity, energy, anomalies
- **HarvestRecord** — resource harvests linked to scans
- **Session** — timed exploration sessions
- **MarketplaceListing** — ship NFT listings, sales, royalties
- **DEXOffer** — resource DEX offers
- **TradeRecord** — order book trade executions
- **Alliance** — player alliances with treasury tracking
- **AllianceMember** — membership records and contributions
- **Proposal** — governance proposals with vote tallies
- **VoteRecord** — individual governance votes
- **StakeRecord** — token staking for yield
- **TreasureVault** — time-locked vault deposits
- **Blueprint** — crafted blueprints with rarity
- **Referral** — referral chain tracking
- **TreasuryTransfer** — DAO treasury disbursements

## Query Examples

### Get a player's ships

```graphql
{
  player(id: "GABC...") {
    totalScans
    essenceEarned
    ships(first: 10) {
      id
      shipType
      level
      hull
    }
  }
}
```

### Get active marketplace listings

```graphql
{
  marketplaceListings(where: { active: true }, orderBy: price, first: 20) {
    id
    price
    ship {
      shipType
      level
    }
    seller {
      totalScans
    }
  }
}
```

### Get active governance proposals

```graphql
{
  proposals(where: { status: "Active" }) {
    id
    description
    forVotes
    againstVotes
    votes(first: 50) {
      voter { id }
      direction
      power
    }
  }
}
```

### Get alliance treasury and members

```graphql
{
  alliance(id: "1") {
    name
    treasury
    members {
      player { id totalScans }
      contribution
      joinedAt
    }
  }
}
```

## License

MIT

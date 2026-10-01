# Staging Environment

Staging mirrors production architecture while using isolated data stores and Stellar testnet. It is the required pre-release proving ground for infrastructure, contract, backend, and frontend changes.

## Parity

| Area | Production | Staging |
|---|---|---|
| Stellar network | Mainnet | Testnet |
| Database | Dedicated PostgreSQL | Dedicated PostgreSQL with anonymized refresh |
| Cache | Dedicated Redis | Dedicated Redis |
| Monitoring | Production Grafana and alerts | Separate Grafana workspace and alert route |
| Access | Public production endpoints | Team CIDR or VPN only |
| Deployment | Blue-green/canary | Same process, lower traffic scale |

## Creation

```bash
cd infrastructure/environments/staging
terraform init
terraform plan -var-file=staging.tfvars
terraform apply -var-file=staging.tfvars
```

Required variables:

- `vpc_id`
- `subnet_ids`
- `db_password`
- `allowed_team_cidrs`

## Data Refresh

Refresh jobs must copy production data into staging only after anonymization:

- replace wallet display aliases with deterministic pseudonyms
- remove email, IP address, device identifiers, and support notes
- scramble free-text user content unless the record is marked public
- keep aggregate economic and gameplay distributions intact for realistic load tests

## Release Gate

Every major change should pass this sequence before production promotion:

1. Deploy to staging using the same artifact that would go to production.
2. Run smoke tests against API health, websocket subscription, frontend load, contract read simulation, and database connectivity.
3. Run a load test using `tests/load/` at the planned release traffic level.
4. Verify staging Grafana dashboards and alert routes.
5. Rehearse rollback or blue-green traffic flip.
6. Record known staging/production differences in the release notes.

The GitHub Actions staging deployment file is intentionally not added in this PR because the active GitHub token cannot push workflow changes.

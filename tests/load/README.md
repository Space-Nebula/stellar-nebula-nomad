# Load Testing Suite

This directory contains load testing scripts for the Stellar Nebula Nomad contract to aid in capacity planning and bottleneck identification.

## Prerequisites

- Install [k6](https://k6.io/docs/get-started/installation/)

## Running the Stress Test

The `k6_stress_test.js` script simulates concurrent read traffic (such as fetching events) against a specified RPC endpoint.

To execute the test, export the contract ID and run:

```bash
export CONTRACT_ID="C..."
# Optional: export RPC_URL="https://your-custom-rpc.local"
k6 run tests/load/k6_stress_test.js
```

### Metrics and Thresholds
- By default, the script ramps up to 20 concurrent Virtual Users (VUs).
- The p95 response time threshold is set to `< 500ms`. If the RPC nodes take longer, the test will be marked as failed.

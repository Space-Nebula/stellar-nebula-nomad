import http from 'k6/http';
import { check, sleep } from 'k6';

// Stellar Nebula Nomad - K6 Load Testing Suite
// This script simulates concurrent read traffic against the RPC endpoint.

export const options = {
  stages: [
    { duration: '30s', target: 20 },  // ramp up to 20 users
    { duration: '1m', target: 20 },   // stay at 20 users for 1 minute
    { duration: '30s', target: 0 },   // ramp down to 0 users
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'], // 95% of requests must complete below 500ms
  },
};

export default function () {
  const rpcUrl = __ENV.RPC_URL || 'https://soroban-testnet.stellar.org';
  const contractId = __ENV.CONTRACT_ID;

  if (!contractId) {
    console.error('CONTRACT_ID environment variable is required');
    return;
  }

  // Simulate a getEvents call using generic RPC payload
  const payload = JSON.stringify({
    jsonrpc: '2.0',
    id: 8675309,
    method: 'getEvents',
    params: {
      startLedger: 1,
      filters: [
        {
          type: 'contract',
          contractIds: [contractId],
        }
      ],
      pagination: { limit: 10 }
    }
  });

  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
  };

  const res = http.post(rpcUrl, payload, params);

  check(res, {
    'is status 200': (r) => r.status === 200,
    'has no rpc error': (r) => {
      try {
        const body = JSON.parse(r.body);
        return !body.error;
      } catch (e) {
        return false;
      }
    }
  });

  sleep(1);
}

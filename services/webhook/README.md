# Webhook Delivery Service

Real-time webhook delivery system for Stellar Nebula contract events.

## Features

- Event filtering and routing
- Automatic retry with exponential backoff
- HMAC signature verification
- Delivery tracking and statistics
- Webhook health monitoring

## Setup

1. Install dependencies:

```bash
npm install
```

2. Configure environment variables:

```bash
DATABASE_URL=postgresql://localhost/webhooks
STELLAR_RPC_URL=https://soroban-testnet.stellar.org
CONTRACT_ID=your_contract_id
PORT=3000
```

3. Start the service:

```bash
npm run dev
```

## API Endpoints

### Register Webhook

```http
POST /webhooks
Content-Type: application/json

{
  "url": "https://your-app.com/webhook",
  "events": ["ship_minted", "resources_harvested"],
  "filters": [
    {
      "field": "data.owner",
      "operator": "eq",
      "value": "GXXXXXX"
    }
  ]
}
```

### Update Webhook

```http
PATCH /webhooks/:id
Content-Type: application/json

{
  "active": false
}
```

### Delete Webhook

```http
DELETE /webhooks/:id
```

### Get Webhook Statistics

```http
GET /webhooks/:id/stats
```

## Webhook Payload

Each webhook delivery includes:

```json
{
  "event": {
    "type": "ship_minted",
    "contractId": "CXXXXXX",
    "ledger": 12345,
    "txHash": "abc123...",
    "timestamp": "2024-01-01T00:00:00Z",
    "data": {
      "owner": "GXXXXXX",
      "shipId": "1",
      "shipType": "Explorer"
    }
  },
  "webhookId": "webhook-id",
  "deliveryId": "delivery-id",
  "timestamp": "2024-01-01T00:00:00Z",
  "signature": "hmac-sha256-signature"
}
```

## Signature Verification

Verify webhook authenticity using HMAC-SHA256:

```javascript
const crypto = require("crypto");

function verifySignature(payload, secret, signature) {
  const data = JSON.stringify({
    event: payload.event,
    webhookId: payload.webhookId,
    deliveryId: payload.deliveryId,
    timestamp: payload.timestamp,
  });

  const expectedSignature = crypto
    .createHmac("sha256", secret)
    .update(data)
    .digest("hex");

  return crypto.timingSafeEqual(
    Buffer.from(signature),
    Buffer.from(expectedSignature),
  );
}
```

## Event Filters

Supported operators:

- `eq`: Equal
- `neq`: Not equal
- `gt`: Greater than
- `lt`: Less than
- `contains`: String contains

## Retry Logic

Failed deliveries are retried with exponential backoff:

- Attempt 1: Immediate
- Attempt 2: 1 second delay
- Attempt 3: 5 seconds delay
- Attempt 4: 15 seconds delay

Webhooks with 10+ consecutive failures are automatically deactivated.

## License

MIT

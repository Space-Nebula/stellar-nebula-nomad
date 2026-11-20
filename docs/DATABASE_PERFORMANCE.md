# Database performance

Query performance changes are limited to indexing, bounded connection pools, explicit projections, and short-lived query caching.

- `migrations/001_query_performance.sql` adds indexes for active devices, notification preference lookups, active webhooks, webhook delivery history, and event/webhook correlation.
- Notification and webhook services use bounded `pg.Pool` configuration.
- Webhook reads select explicit columns and cache active webhooks for 30 seconds.
- Webhook writes invalidate the active-webhook cache.
- Delivery history selects explicit columns and relies on the `(webhook_id, status, timestamp DESC)` index.

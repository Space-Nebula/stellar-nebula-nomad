# Asset delivery

Static assets are served through CloudFront and an S3 origin locked down with origin access control.

- `infrastructure/modules/cdn/main.tf` creates the versioned S3 bucket, origin access control, bucket policy, and compressed CloudFront distribution.
- The module is wired into dev, staging, and production environments and exports `cdn_url`.
- `frontend/src/utils/assets.ts` builds CDN URLs with automatic format selection, responsive widths, quality parameters, lazy loading, and async decoding.
- `frontend/src/services/cache.ts` provides a one-hour frontend asset cache.

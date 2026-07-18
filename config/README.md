# Runtime configuration

## Recommended cookie skip rules

`cookie-skip-rules.json` is the public, data-only catalog used by installed Youwee clients.
It contains domain or domain/path prefixes only; it must never contain cookies, profile paths,
proxy credentials, regular expressions, or executable content.

When changing the catalog:

1. Validate it against `cookie-skip-rules.schema.json`.
2. Update `revision` and `updatedAt`.
3. Keep rules normalized and review whether skipping authentication can expose private-only URLs.
   Add a rule only when cookies consistently break public extraction; do not use the catalog to
   bypass authentication by default when cookies can expose better formats or metadata.
4. Push the change to `main`. Clients refresh at most once every 24 hours and retain the last
   valid cache when the network or catalog is unavailable.

Users can disable recommended rules or add personal rules on their own device. Personal rules
remain local and are never uploaded.

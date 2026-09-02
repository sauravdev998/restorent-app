# Error & Crash Monitoring Research - 2026-08-08

## Quick Landscape

| Tool | Type | Free Tier | First Paid | EU Region | Rust SDK | React SDK | Self-Host |
|------|------|-----------|------------|-----------|----------|-----------|-----------|
| **Sentry** | SaaS | 5k errors/mo, 30d retention | $26/mo Team | US only* | v0.49.1, tracing+tower | v8.47+, React 19+Router 7 | Yes, 16GB RAM |
| **GlitchTip** | Self-hosted | — | Free/OSS | Yes* | Sentry SDK compatible | Sentry SDK compatible | 512MB RAM |
| **Bugsnag** | SaaS | 7.5k errors/mo | ~$22/mo | Unknown | SDK available | SDK available | No |
| **Rollbar** | SaaS | 5k events/mo | $9/mo | GDPR OK, US infra | SDK available | SDK available | No |
| **Honeybadger** | SaaS | Trial only | $49/mo (uptime+check-in bundled) | Unknown | SDK available | SDK available | No |
| **AWS CloudWatch** | Native | Included | Usage-based | eu-west-1 | Via structured logs | Via JavaScript logs | N/A |

**EU note:** Sentry US-operated, no published EU region. GlitchTip self-hosted = full control. Rollbar US infrastructure despite GDPR compliance.

## Sentry Specifics (Likely Pick)

- **Version:** sentry-rust v0.49.1 (Aug 3, 2026), @sentry/react integrated with React 19 error hooks
- **SDKs:** Official sentry-rust with sentry-tracing (for `tracing` integration) + sentry-tower (for Axum layers)
- **React:** @sentry/react supports React Router 7 via `Sentry.reactRouterV7BrowserTracingIntegration()`
- **Source maps:** @sentry/vite-plugin automates upload; requires auth token in CI
- **EU region:** Not documented; Sentry operates from US

## GlitchTip Status

- **Active:** Yes, v6 released Feb 2026 with improved stacktraces
- **Cost:** Self-hosted only, OSS (GitHub)
- **Compatibility:** Accepts sentry-rust + @sentry/react events (Sentry protocol compatible)
- **Resource footprint:** 512 MB RAM vs Sentry self-hosted 16GB+
- **Data residency:** Full control if self-hosted in EU

## Freshness Notes (Last ~12 Months)

1. **Highlight.io discontinued:** Acquired by LaunchDarkly; product sunset Feb 28, 2026 — no longer an option
2. **React 19 support:** @sentry/react added first-class React 19 error hook integration (early 2026)
3. **React Router 7:** Sentry added dedicated Router 7 instrumentation via `reactRouterV7BrowserTracingIntegration()`
4. **Pricing stable:** No major changes reported; Sentry free tier remains 5k/mo, Rollbar $9/mo
5. **GlitchTip growth:** Maintained actively, resource efficiency edge over Sentry self-hosted holds

## AWS Native Path

CloudWatch Logs → metric filters + alarms. Viable for cost-sensitive teams already on AWS, but lacks error grouping and source map support unless layers added.

## Confirmed Sources

- [Sentry pricing (SaaS tiers, free 5k/30d)](https://sentry.io/pricing/) — Aug 2026
- [sentry-rust docs (v0.49.1, tracing+tower)](https://docs.rs/sentry/latest/sentry/) — Aug 2026
- [sentry-tracing crate (tracing integration)](https://crates.io/crates/sentry-tracing) — current
- [sentry-tower middleware (Axum support)](https://crates.io/crates/sentry-tower) — current
- [Sentry React guide (React 19, Router 7 support)](https://docs.sentry.io/platforms/javascript/guides/react/) — Aug 2026
- [@sentry/vite-plugin (source map upload)](https://www.npmjs.com/package/@sentry/vite-plugin) — current
- [GlitchTip v6 Feb 2026 release, v512MB efficiency](https://uptrace.dev/comparisons/sentry-alternatives) — cross-source verified
- [Highlight.io discontinued, LaunchDarkly acquisition Feb 2026](https://cubeapm.com/blog/highlight-io-pricing-and-review/) — confirmed

## Recommendation

For this stack (Rust+Axum, React 19, EU GDPR, cost-sensitive):
1. **Sentry SaaS:** Easiest; official SDKs fully current; $26/mo team tier. No EU region published—data goes to US. Auth token needed for source maps in CI.
2. **GlitchTip self-hosted:** Lower ops cost if team has time; same SDKs; full EU data control. 512 MB footprint practical for small team.
3. **AWS CloudWatch:** Cheapest if already on ECS/CloudWatch; lacks error grouping/source maps without custom work.

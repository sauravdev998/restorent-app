# Infrastructure (AWS CDK, TypeScript)

## Overview

The whole platform in one CDK stack: network, RDS Postgres, an ECR repository, a Fargate
service behind an Application Load Balancer, and an S3 bucket behind CloudFront that also
routes `/api/*` to the load balancer. A pnpm workspace package. **Nothing here has been
deployed.** It synthesises and it encodes the decisions in spec 0001, so treat every number as
a starting point and expect the first deploy to teach you something.

## Key files

| File | Owns |
|---|---|
| `lib/platform-stack.ts` | The entire stack, and the reasoning behind the settings that look like details |
| `bin/app.ts` | One environment, production. Region defaults to `eu-west-1` |
| `cdk.json` | Runs the app through `tsx`, plus the CDK feature flags |

## Commands

Run from the repository root.

```bash
pnpm synth                          # or pnpm --filter infra synth
pnpm --filter infra diff
pnpm --filter infra deploy
pnpm --filter infra typecheck
```

Pass the image to run with CDK context: `pnpm --filter infra deploy -c imageTag=<sha>`.
It defaults to `latest`.

## Conventions

- **One stack, one environment.** Production plus each engineer's machine. No staging until a real restaurant depends on the system.
- **Secrets come from SSM Parameter Store**, injected into the task definition by ECS. Nothing sensitive in the repository, in a build log, or in the image.
- **The database keeps its data on stack removal**: `deletionProtection`, a `RETAIN` removal policy, and seven days of backups. The web bucket retains too.
- **Neither the application nor migrations use the RDS master user.** The owner role and `app_api` are created by hand on top of it, matching `api/scripts/init-roles.sql` locally.

## Gotchas

- **Three numbers keep a kitchen screen connected, and they live in two languages.** The load balancer `idleTimeout` is 300 seconds and the CloudFront origin `readTimeout` is 60 seconds, both here; the server's comment heartbeat is 15 seconds, in `api/src/presentation/handlers/events.rs`. Nothing links them. Lower one here and no build goes red, the screens simply start dropping. The API test that pins the relationship compares against copies of these numbers.
- **No NAT gateway, so tasks run in public subnets** with `assignPublicIp`. Nothing reaches them except the load balancer's security group. A NAT gateway would cost more than the rest of the stack combined.
- **arm64 (Graviton), so the Docker build must target the same architecture**: `docker buildx build --platform linux/arm64 -f api/Dockerfile .` from the repository root.
- **Caching is disabled on the `/api/*` behaviour, and compression is off there.** Caching an event stream means delivering nothing; caching an authenticated response means delivering it to the wrong restaurant.
- **The health check hits `/api/health`**, which answers 200 only when the pool answers and the Postgres listen connection is alive. A liveness only check would leave a task in service while it silently delivered nothing.
- **`minHealthyPercent: 100` and a 60 second deregistration delay** give open streams time to drain, but a rolling deploy still closes every stream at once and every screen reconnects. Deploy between services.
- **403 and 404 both return `index.html`.** Correct for a single page app; feature 19's marketing page will need its own answer for search engines.

## Related specs

- [0001 stack and architecture](../docs/specs/0001-stack-and-architecture/index.md)

_Drafted by /audit from the repo, worth a quick human pass. Edit freely: once a line stops matching this draft, later runs treat it as curated and will flag rather than overwrite it._

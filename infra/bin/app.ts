#!/usr/bin/env node
import { App } from 'aws-cdk-lib'

import { PlatformStack } from '../lib/platform-stack'

const app = new App()

// Production plus each engineer's local machine. No staging yet: pre launch and
// solo, a staging environment mostly doubles the bill and the infrastructure to
// maintain. Add one when a real restaurant depends on the system.
new PlatformStack(app, 'RestaurantPlatformProduction', {
  env: {
    account: process.env.CDK_DEFAULT_ACCOUNT,
    region: process.env.CDK_DEFAULT_REGION ?? 'eu-west-1',
  },
  description: 'Restaurant operations platform: API, database, and web hosting.',
})

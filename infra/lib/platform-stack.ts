import { CfnOutput, Duration, RemovalPolicy, Stack, type StackProps } from 'aws-cdk-lib'
import * as cloudfront from 'aws-cdk-lib/aws-cloudfront'
import * as origins from 'aws-cdk-lib/aws-cloudfront-origins'
import * as ec2 from 'aws-cdk-lib/aws-ec2'
import * as ecr from 'aws-cdk-lib/aws-ecr'
import * as ecs from 'aws-cdk-lib/aws-ecs'
import * as elbv2 from 'aws-cdk-lib/aws-elasticloadbalancingv2'
import * as logs from 'aws-cdk-lib/aws-logs'
import * as rds from 'aws-cdk-lib/aws-rds'
import * as s3 from 'aws-cdk-lib/aws-s3'
import * as ssm from 'aws-cdk-lib/aws-ssm'
import type { Construct } from 'constructs'

/**
 * The whole platform, in one stack.
 *
 * NOT YET DEPLOYED. This encodes the decisions spec 0001 made, and it
 * synthesises, but nothing here has met a real AWS account. Treat every number
 * as a starting point and expect the first deploy to teach you something.
 *
 * The settings below that look like details are not:
 *
 * - **No NAT gateway.** Fargate tasks in private subnets need one, at roughly
 *   32 US dollars a month plus data charges, which would cost more than
 *   everything else in this stack combined. Tasks run in public subnets with no
 *   inbound access except from the load balancer's security group.
 * - **A 300 second load balancer idle timeout.** The default is 60 seconds and
 *   it would close a quiet event stream, so a kitchen screen would drop every
 *   minute during a lull. The server also sends a comment heartbeat every 15
 *   seconds.
 * - **The health check hits `/api/health`.** That endpoint returns 200 only when
 *   the pool answers and the Postgres listen connection is alive. A liveness
 *   only check would leave a task in service forever while it silently
 *   delivered nothing.
 * - **arm64.** Roughly 20 percent cheaper for identical work. The Docker build
 *   must target the same architecture.
 * - **`/api/*` is routed from the same CloudFront distribution.** One origin
 *   means no CORS to configure and the session cookie simply works. Caching is
 *   disabled on that behaviour, because caching an event stream would mean
 *   delivering nothing.
 */
export class PlatformStack extends Stack {
  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props)

    // Which image tag to run. Continuous integration passes the commit sha.
    const imageTag = this.node.tryGetContext('imageTag') ?? 'latest'

    // ---------------------------------------------------------------------
    // Network. Two availability zones, and deliberately no NAT gateway.
    // ---------------------------------------------------------------------
    const vpc = new ec2.Vpc(this, 'Vpc', {
      maxAzs: 2,
      natGateways: 0,
      subnetConfiguration: [
        { name: 'public', subnetType: ec2.SubnetType.PUBLIC, cidrMask: 24 },
        { name: 'database', subnetType: ec2.SubnetType.PRIVATE_ISOLATED, cidrMask: 24 },
      ],
    })

    // ---------------------------------------------------------------------
    // Database. Single instance, smallest Graviton burstable, isolated subnets.
    // Single instance means a maintenance window is downtime. Acceptable before
    // real restaurants depend on it; move to Multi AZ when one does.
    // ---------------------------------------------------------------------
    const database = new rds.DatabaseInstance(this, 'Database', {
      engine: rds.DatabaseInstanceEngine.postgres({
        version: rds.PostgresEngineVersion.of('17.4', '17'),
      }),
      instanceType: ec2.InstanceType.of(ec2.InstanceClass.BURSTABLE4_GRAVITON, ec2.InstanceSize.MICRO),
      vpc,
      vpcSubnets: { subnetType: ec2.SubnetType.PRIVATE_ISOLATED },
      allocatedStorage: 20,
      maxAllocatedStorage: 100,
      multiAz: false,
      // A restaurant's billing history is not something to lose.
      backupRetention: Duration.days(7),
      deletionProtection: true,
      removalPolicy: RemovalPolicy.RETAIN,
      storageEncrypted: true,
      // The master user is used by neither the application nor migrations. The
      // owner and app_api roles are created by hand on top of it, per spec 0001.
      credentials: rds.Credentials.fromGeneratedSecret('rds_master'),
    })

    // ---------------------------------------------------------------------
    // Where the API image lives.
    // ---------------------------------------------------------------------
    const repository = new ecr.Repository(this, 'ApiRepository', {
      repositoryName: 'restaurant-api',
      imageScanOnPush: true,
      lifecycleRules: [{ maxImageCount: 20, description: 'Keep the last 20 images.' }],
    })

    // ---------------------------------------------------------------------
    // Compute.
    // ---------------------------------------------------------------------
    const cluster = new ecs.Cluster(this, 'Cluster', { vpc, containerInsightsV2: undefined })

    const taskDefinition = new ecs.FargateTaskDefinition(this, 'ApiTask', {
      cpu: 512,
      memoryLimitMiB: 1024,
      runtimePlatform: {
        cpuArchitecture: ecs.CpuArchitecture.ARM64,
        operatingSystemFamily: ecs.OperatingSystemFamily.LINUX,
      },
    })

    // Secrets and configuration come from SSM Parameter Store, injected by ECS.
    // No secret in the repository and none in a build log. Standard parameters
    // are free.
    const databaseUrl = ssm.StringParameter.fromSecureStringParameterAttributes(this, 'DatabaseUrl', {
      parameterName: '/restaurant/production/DATABASE_URL',
    })

    taskDefinition.addContainer('api', {
      image: ecs.ContainerImage.fromEcrRepository(repository, imageTag),
      portMappings: [{ containerPort: 8080 }],
      environment: {
        APP_ENV: 'production',
        APP_HOST: '0.0.0.0',
        APP_PORT: '8080',
        RUST_LOG: 'api=info,tower_http=info,warn',
      },
      secrets: {
        DATABASE_URL: ecs.Secret.fromSsmParameter(databaseUrl),
      },
      logging: ecs.LogDrivers.awsLogs({
        streamPrefix: 'api',
        logRetention: logs.RetentionDays.ONE_MONTH,
      }),
      // The container's own view of the same condition the load balancer checks.
      healthCheck: {
        command: ['CMD-SHELL', 'curl -fsS http://localhost:8080/api/health || exit 1'],
        interval: Duration.seconds(30),
        timeout: Duration.seconds(5),
        retries: 3,
        startPeriod: Duration.seconds(30),
      },
    })

    const service = new ecs.FargateService(this, 'ApiService', {
      cluster,
      taskDefinition,
      desiredCount: 1,
      // Public subnets, because avoiding a NAT gateway is worth more than the
      // marginal comfort of a private subnet here. Nothing reaches these tasks
      // except the load balancer's security group.
      vpcSubnets: { subnetType: ec2.SubnetType.PUBLIC },
      assignPublicIp: true,
      circuitBreaker: { rollback: true },
      // Long enough for open event streams to finish, short enough that a
      // deploy is not a coffee break. A rolling deploy closes every stream at
      // once and every screen reconnects, so deploy between services.
      minHealthyPercent: 100,
    })

    database.connections.allowDefaultPortFrom(service, 'the api tasks')

    // ---------------------------------------------------------------------
    // Load balancer.
    // ---------------------------------------------------------------------
    const loadBalancer = new elbv2.ApplicationLoadBalancer(this, 'LoadBalancer', {
      vpc,
      internetFacing: true,
      // The default 60 seconds would close a quiet event stream.
      idleTimeout: Duration.seconds(300),
    })

    const listener = loadBalancer.addListener('Http', { port: 80, open: true })

    listener.addTargets('ApiTargets', {
      port: 8080,
      protocol: elbv2.ApplicationProtocol.HTTP,
      targets: [service],
      healthCheck: {
        path: '/api/health',
        interval: Duration.seconds(15),
        timeout: Duration.seconds(5),
        healthyThresholdCount: 2,
        unhealthyThresholdCount: 3,
      },
      // Streams need time to drain when a task is being replaced.
      deregistrationDelay: Duration.seconds(60),
    })

    // ---------------------------------------------------------------------
    // Static hosting. One origin for the whole product.
    // ---------------------------------------------------------------------
    const siteBucket = new s3.Bucket(this, 'WebBucket', {
      blockPublicAccess: s3.BlockPublicAccess.BLOCK_ALL,
      encryption: s3.BucketEncryption.S3_MANAGED,
      enforceSSL: true,
      removalPolicy: RemovalPolicy.RETAIN,
    })

    const distribution = new cloudfront.Distribution(this, 'Cdn', {
      defaultRootObject: 'index.html',
      defaultBehavior: {
        origin: origins.S3BucketOrigin.withOriginAccessControl(siteBucket),
        viewerProtocolPolicy: cloudfront.ViewerProtocolPolicy.REDIRECT_TO_HTTPS,
        compress: true,
      },
      additionalBehaviors: {
        '/api/*': {
          origin: new origins.LoadBalancerV2Origin(loadBalancer, {
            protocolPolicy: cloudfront.OriginProtocolPolicy.HTTP_ONLY,
            // Must exceed the server's 15 second heartbeat, or CloudFront
            // hangs up on a quiet stream before the next comment arrives.
            readTimeout: Duration.seconds(60),
            keepaliveTimeout: Duration.seconds(60),
          }),
          viewerProtocolPolicy: cloudfront.ViewerProtocolPolicy.HTTPS_ONLY,
          allowedMethods: cloudfront.AllowedMethods.ALLOW_ALL,
          // Caching an event stream would mean delivering nothing, and caching
          // an authenticated API response would mean delivering it to the wrong
          // restaurant.
          cachePolicy: cloudfront.CachePolicy.CACHING_DISABLED,
          originRequestPolicy: cloudfront.OriginRequestPolicy.ALL_VIEWER,
          compress: false,
        },
      },
      // A single page app: any unmatched path is a client route, not a missing
      // file. Feature 19's marketing page needs its own answer for search
      // engines, most likely pre rendering that one page.
      errorResponses: [
        { httpStatus: 403, responseHttpStatus: 200, responsePagePath: '/index.html' },
        { httpStatus: 404, responseHttpStatus: 200, responsePagePath: '/index.html' },
      ],
    })

    new CfnOutput(this, 'SiteUrl', { value: `https://${distribution.distributionDomainName}` })
    new CfnOutput(this, 'EcrRepositoryUri', { value: repository.repositoryUri })
    new CfnOutput(this, 'WebBucketName', { value: siteBucket.bucketName })
    new CfnOutput(this, 'LoadBalancerDns', { value: loadBalancer.loadBalancerDnsName })
  }
}

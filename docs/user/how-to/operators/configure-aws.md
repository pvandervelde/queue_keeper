# Configure AWS Services

This guide provisions the AWS resources Queue-Keeper depends on when deployed on AWS: SQS (for bot queues), S3 (for raw payload persistence), and Secrets Manager (for webhook secrets). All commands use the AWS CLI.

## Prerequisites

- AWS CLI 2.x installed and configured (`aws configure` or environment variables)
- IAM permissions to create SQS queues, S3 buckets, and Secrets Manager secrets
- An IAM role or user for Queue-Keeper's workload (container, Lambda, or EC2 task role)

---

## 1 — AWS SQS

### Create a queue for each bot

Repeat for every bot in `bot-config.yaml`. Use a FIFO queue when the bot has `ordered: true`, a standard queue when `ordered: false`.

**Ordered bot (`ordered: true`) — FIFO queue:**

```bash
aws sqs create-queue \
  --queue-name queue-keeper-task-tactician.fifo \
  --attributes \
    FifoQueue=true,\
    ContentBasedDeduplication=true,\
    VisibilityTimeout=300,\
    MessageRetentionPeriod=1209600,\
    RedrivePolicy='{"deadLetterTargetArn":"<DLQ_ARN>","maxReceiveCount":"10"}'
```

!!! tip "Dead-letter queues"
    Create a companion DLQ first (also a FIFO queue), then reference its ARN in `RedrivePolicy`. Replace `<DLQ_ARN>` with the result of creating `queue-keeper-task-tactician-dlq.fifo`.

**Unordered bot (`ordered: false`) — standard queue:**

```bash
aws sqs create-queue \
  --queue-name queue-keeper-notifications \
  --attributes \
    VisibilityTimeout=300,\
    MessageRetentionPeriod=1209600,\
    RedrivePolicy='{"deadLetterTargetArn":"<DLQ_ARN>","maxReceiveCount":"10"}'
```

### Grant Queue-Keeper send access

Attach an IAM policy to Queue-Keeper's task role that allows `sqs:SendMessage` on each bot queue:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "sqs:SendMessage",
        "sqs:GetQueueAttributes"
      ],
      "Resource": "arn:aws:sqs:us-east-1:123456789012:queue-keeper-*"
    }
  ]
}
```

Grant bot consumers `sqs:ReceiveMessage`, `sqs:DeleteMessage`, and `sqs:ChangeMessageVisibility` on their specific queues.

### Queue-Keeper `service.yaml`

```yaml
queue:
  provider: aws_sqs
  region: us-east-1
```

Queue-Keeper uses the standard AWS SDK credential chain (ECS task role, EC2 instance profile, environment variables). Do not embed credentials in the configuration file.

---

## 2 — AWS S3

S3 stores raw webhook payloads for audit and replay.

### Create the bucket

```bash
aws s3api create-bucket \
  --bucket queue-keeper-webhooks-prod \
  --region us-east-1
```

For regions other than `us-east-1`, add `--create-bucket-configuration LocationConstraint=<region>`.

### Block public access

```bash
aws s3api put-public-access-block \
  --bucket queue-keeper-webhooks-prod \
  --public-access-block-configuration \
    BlockPublicAcls=true,\
    IgnorePublicAcls=true,\
    BlockPublicPolicy=true,\
    RestrictPublicBuckets=true
```

### Enable versioning (recommended for compliance)

```bash
aws s3api put-bucket-versioning \
  --bucket queue-keeper-webhooks-prod \
  --versioning-configuration Status=Enabled
```

### Grant Queue-Keeper access

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:PutObject",
        "s3:GetObject",
        "s3:ListBucket"
      ],
      "Resource": [
        "arn:aws:s3:::queue-keeper-webhooks-prod",
        "arn:aws:s3:::queue-keeper-webhooks-prod/*"
      ]
    }
  ]
}
```

### `service.yaml` blob storage section

```yaml
blob_storage:
  provider: aws_s3
  bucket: queue-keeper-webhooks-prod
  region: us-east-1
```

---

## 3 — AWS Secrets Manager

Secrets Manager stores webhook signing secrets so they never appear in configuration files or environment variables.

### Create a secret

```bash
aws secretsmanager create-secret \
  --name "queue-keeper/github-webhook-secret" \
  --secret-string "$(openssl rand -base64 32)"
```

### Grant Queue-Keeper read access

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "secretsmanager:GetSecretValue"
      ],
      "Resource": "arn:aws:secretsmanager:us-east-1:123456789012:secret:queue-keeper/*"
    }
  ]
}
```

### Reference the secret in `service.yaml`

```yaml
providers:
  - id: "github"
    require_signature: true
    secret:
      type: aws_secrets_manager
      secret_name: "queue-keeper/github-webhook-secret"
      region: "us-east-1"
```

---

## 4 — Supported queue backends

Queue-Keeper currently supports:

| Provider | Ordered delivery | Configuration key |
|---|---|---|
| **Azure Service Bus** | Session-based FIFO | `azure_service_bus` |
| **AWS SQS** | FIFO queues | `aws_sqs` |
| **In-memory** | No (dev only) | `in_memory` |

RabbitMQ and NATS are **not** currently supported by the `queue-runtime` library that Queue-Keeper depends on.

---

## Next steps

- [Deploy on Kubernetes](deploy-kubernetes.md) — Kubernetes manifests with AWS IAM Roles for Service Accounts (IRSA)
- [Configure Webhook Providers](configure-providers.md) — setting up GitHub and generic providers
- [Rotate Secrets](rotate-secrets.md) — rotating webhook secrets in Secrets Manager

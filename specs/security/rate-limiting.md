# Rate Limiting and DDoS Protection Strategy

## Overview

Queue-Keeper's rate limiting strategy protects against abuse while accommodating legitimate GitHub webhook traffic patterns. The multi-layered approach separates infrastructure protection from application-level controls, ensuring service availability during attack scenarios while maintaining webhook processing SLA.

## Multi-Layer Protection Strategy

### Layer 1: Infrastructure Protection (Azure Front Door)

**Purpose**: Filter malicious traffic before it reaches Queue-Keeper, reducing resource consumption and attack surface.

**Protection Capabilities**:

| Protection Layer | Attack Types | Rationale |
|------------------|--------------|-----------|
| **DDoS Protection Standard** | Volumetric attacks (Layer 3/4) | Azure-native protection with automatic scaling |
| **Web Application Firewall** | HTTP floods, bot traffic (Layer 7) | Pattern-based detection of malicious requests |
| **Geographic Filtering** | Region-based attacks | Block traffic from non-GitHub regions if needed |
| **Adaptive Thresholds** | Sophisticated attacks | Machine learning detects evolving attack patterns |

**Rate Limiting Approach**:

- **Global Organization Limit**: 10,000 requests/minute (generous for legitimate GitHub usage)
- **Per-IP Limits**: 1,000 requests/minute per source IP (accommodates CI/CD bursts)
- **Burst Handling**: 2,000 request burst capacity for legitimate traffic spikes
- **GitHub IP Allowlist**: Bypass limits for verified GitHub webhook sources

### Layer 2: Application-Level Rate Limiting

**Purpose**: Provide fine-grained control over webhook processing with awareness of GitHub traffic patterns and bot subscription context.

**Algorithm Selection Strategy**:

| Algorithm | Use Case | Rationale |
|-----------|----------|-----------|
| **Token Bucket** | Handling CI/CD bursts | Allows short-term burst above average rate |
| **Sliding Window** | Smooth rate enforcement | Prevents gaming fixed window boundaries |
| **Fixed Window** | Simple per-hour budgets | Easy to understand and implement |

**Key Design Decisions**:

- **Burst Accommodation**: Token bucket algorithm chosen for handling legitimate webhook bursts
- **Context Awareness**: Different limits based on bot subscription, repository activity, IP reputation
- **Graceful Degradation**: Rate limiting failures do not block webhook processing
- **Observability**: All rate limiting decisions logged for analysis and tuning

```

## Rate Limiting Categories

### IP-Based Rate Limiting Strategy

**IP Classification Approach**:

| Classification | Request Rate | Purpose | Upgrade/Downgrade Criteria |
|----------------|-------------|---------|----------------------------|
| **Whitelisted** | No limits | Known GitHub IP ranges | Static GitHub IP range list |
| **Normal** | 100/minute, 1000/hour | Standard legitimate traffic | Default classification |
| **Suspicious** | 10/minute, 100/hour | Flagged but not blocked IPs | 3+ behavioral indicators |
| **Blocked** | 0/minute | Malicious traffic | Authentication failures, attack patterns |

**Behavioral Analysis Strategy**:
Queue-Keeper analyzes request patterns to identify potentially malicious sources before they cause damage.

**Suspicious Behavior Indicators**:
- **High Error Rate**: >50% errors in 5-minute window indicates scanning/probing
- **Bot-like Timing**: Overly regular request intervals suggest automated attacks
- **Invalid User Agents**: Missing or non-standard user agent headers
- **Signature Failures**: Repeated webhook signature validation failures (weighted higher)

**IP Classification Decisions**:
- **GitHub IP Allowlist**: Bypass all limits for verified GitHub webhook sources
- **Pattern-Based Suspicion**: 3+ behavioral indicators trigger rate reduction
- **Security Event Escalation**: Authentication failures and signature violations lead to blocking
```

### Repository-Based Rate Limiting Strategy

**Repository Classification Strategy**:

| Repository Type | Webhook Rate | Rationale | Classification Criteria |
|----------------|-------------|-----------|------------------------|
| **Active Repository** | 1000/minute, 10K/hour | High-activity, trusted repos | 30+ days old, 10+ contributors, 100+ daily events |
| **Normal Repository** | 100/minute, 1K/hour | Standard repository activity | 7+ days old, 2+ contributors, 10+ daily events |
| **New Repository** | 10/minute, 100/hour | Conservative limits during probation | <7 days old or minimal activity |

**Repository Context Awareness**:

- **CI/CD Burst Handling**: Higher burst allowance for active repositories to accommodate deployment pipelines
- **Probationary Period**: New repositories start with conservative limits, graduate based on legitimate usage patterns
- **Bot Subscription Context**: Limits applied per bot subscription rather than globally per repository

### Security-Based Rate Limiting Strategy

**Authentication Failure Protection Strategy**:
Progressive response to repeated authentication failures, escalating from warnings to complete blocks.

**Security Response Escalation**:

| Failure Count (5min/1hour) | Response | Duration | Rationale |
|---------------------------|----------|----------|-----------|
| **<10 failures** | Allow with logging | N/A | Single mistakes or occasional errors |
| **10-50 failures** | Rate restriction | 1 hour | Potential brute force, reduce access |
| **>50 failures** | Complete block | 24 hours | Clear attack pattern, prevent further damage |

## Abuse Detection Strategy

### Attack Pattern Recognition

**Detection Approach**:
Queue-Keeper identifies attack patterns through behavioral analysis rather than simple thresholds, reducing false positives while catching sophisticated attacks.

**Attack Pattern Categories**:

| Attack Type | Indicators | Detection Method | Response |
|-------------|-----------|------------------|----------|
| **Single-Source Volume** | High rate from single IP | Rate spike analysis | IP-based rate limiting |
| **Distributed Attack** | Coordinated multi-IP attack | Geographic and timing correlation | Regional blocking, pattern-based filtering |
| **Signature Bypass** | Invalid webhook signatures | Cryptographic validation failure patterns | Enhanced authentication requirements |
| **Bot Behavior** | Regular timing, suspicious user agents | Statistical timing analysis | User agent filtering, CAPTCHA challenges |

### Traffic Pattern Analysis

**Legitimate vs. Malicious Traffic Characteristics**:

**Legitimate GitHub Webhook Traffic**:

- **Timing**: Event-driven, irregular bursts around development activity
- **User Agent**: Consistent "GitHub-Hookshot/*" pattern
- **Payload**: Valid GitHub webhook schema with proper signatures
- **Source IPs**: GitHub's documented IP ranges
- **Burst Pattern**: 5-50 events in 1-2 minutes during CI/CD, then quiet periods

**Suspicious/Malicious Traffic**:

- **Timing**: Overly regular intervals suggesting automation
- **User Agent**: Missing, non-standard, or rotating user agents
- **Error Rate**: High proportion of 4xx/5xx responses (scanning/probing)
- **Payload**: Malformed requests or probe attempts
- **Geographic**: Unusual geographic distribution not matching GitHub infrastructure

## Rate Limiting Integration Strategy

**Middleware Integration Philosophy**:
Rate limiting checks occur early in the request pipeline but do not block critical webhook processing. Failed rate limit checks result in HTTP 429 responses with appropriate retry headers.

**Check Priority Order**:

1. **IP-based limits** (most restrictive, catches obvious attacks)
2. **Repository-based limits** (context-aware, accommodates legitimate bursts)
3. **Security limits** (authentication failure tracking, prevents brute force)

**Response Headers Strategy**:
All rate limit responses include standard HTTP headers (X-RateLimit-*) to help legitimate clients understand and respect limits.

## Monitoring and Observability

**Key Metrics Strategy**:

| Metric Category | Purpose | Alerting Strategy |
|----------------|---------|------------------|
| **Request Classification** | Track legitimate vs suspicious traffic | Alert on classification accuracy |
| **Rate Limit Violations** | Monitor attack attempts and false positives | Alert on high violation rates |
| **Attack Pattern Detection** | Identify sophisticated attacks early | Alert on high-confidence attack detection |
| **Performance Impact** | Ensure rate limiting doesn't affect SLA | Alert on increased latency |

**Dashboard Strategy**:

- **Traffic Overview**: Real-time view of request rates, blocks, and classifications
- **Security Monitoring**: Authentication failures, blocked IPs, attack pattern detection
- **Performance Impact**: Rate limiting latency, false positive rates

**Alert Strategy**:

- **High Block Rate**: >10% of requests blocked (potential false positives)
- **DDoS Detection**: High-confidence attack pattern detected (immediate response needed)
- **Rate Limiter Errors**: Internal failures affecting rate limiting functionality

This comprehensive rate limiting and DDoS protection strategy ensures Queue-Keeper can distinguish between legitimate GitHub webhook traffic and malicious attacks while maintaining high availability and meeting webhook response SLA requirements.

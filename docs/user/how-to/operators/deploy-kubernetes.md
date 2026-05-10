# Deploy on Kubernetes

This guide deploys Queue-Keeper on Kubernetes using a Deployment, ConfigMap, Secret, and Service. It assumes an Azure Kubernetes Service (AKS) cluster with workload identity or pod-managed identity enabled for Key Vault access.

## Prerequisites

- `kubectl` configured for your cluster
- Azure Service Bus namespace and queues provisioned — see [Configure Azure Services](configure-azure.md)
- Bot configuration ready — see [Add a Bot Subscription](add-bot-subscription.md)
- Container image accessible from the cluster (GitHub Container Registry or a pull-through cache)

---

## Manifests

### ConfigMap — `bot-config.yaml`

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: queue-keeper-bot-config
  namespace: automation
data:
  bot-config.yaml: |
    bots:
      - name: "task-tactician"
        queue: "queue-keeper-task-tactician"
        events:
          - "issues.*"
          - "pull_request.*"
        ordered: true

      - name: "notification-bot"
        queue: "queue-keeper-notifications"
        events: ["*"]
        ordered: false
```

### ConfigMap — `service.yaml`

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: queue-keeper-service-config
  namespace: automation
data:
  service.yaml: |
    server:
      port: 8080
      host: "0.0.0.0"

    logging:
      level: "info"
      format: "json"

    providers:
      - id: "github"
        require_signature: true
        secret:
          type: key_vault
          secret_name: "github-webhook-secret"

    key_vault:
      vault_url: "https://my-vault.vault.azure.net"

    queue:
      azure_service_bus:
        namespace_url: "https://my-namespace.servicebus.windows.net"
        use_managed_identity: true
```

### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: queue-keeper
  namespace: automation
  labels:
    app: queue-keeper
spec:
  replicas: 2
  selector:
    matchLabels:
      app: queue-keeper
  template:
    metadata:
      labels:
        app: queue-keeper
      annotations:
        # AKS workload identity annotation — replace with your client ID
        azure.workload.identity/client-id: "00000000-0000-0000-0000-000000000000"
    spec:
      serviceAccountName: queue-keeper
      containers:
        - name: queue-keeper
          image: ghcr.io/pvandervelde/queue-keeper:latest
          command: ["queue-keeper", "start", "--foreground"]
          ports:
            - containerPort: 8080
          env:
            - name: QUEUE_KEEPER_CONFIG
              value: /config/service.yaml
          volumeMounts:
            - name: service-config
              mountPath: /config/service.yaml
              subPath: service.yaml
              readOnly: true
            - name: bot-config
              mountPath: /config/bot-config.yaml
              subPath: bot-config.yaml
              readOnly: true
          livenessProbe:
            httpGet:
              path: /health
              port: 8080
            initialDelaySeconds: 10
            periodSeconds: 30
            failureThreshold: 3
          readinessProbe:
            httpGet:
              path: /ready
              port: 8080
            initialDelaySeconds: 5
            periodSeconds: 10
            failureThreshold: 3
          resources:
            requests:
              cpu: "250m"
              memory: "256Mi"
            limits:
              cpu: "1000m"
              memory: "512Mi"
      volumes:
        - name: service-config
          configMap:
            name: queue-keeper-service-config
        - name: bot-config
          configMap:
            name: queue-keeper-bot-config
```

### Service

```yaml
apiVersion: v1
kind: Service
metadata:
  name: queue-keeper
  namespace: automation
spec:
  selector:
    app: queue-keeper
  ports:
    - port: 80
      targetPort: 8080
      protocol: TCP
```

### ServiceAccount (AKS workload identity)

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: queue-keeper
  namespace: automation
  annotations:
    azure.workload.identity/client-id: "00000000-0000-0000-0000-000000000000"
```

---

## Apply all manifests

```bash
kubectl apply -f namespace.yaml
kubectl apply -f configmap-bot-config.yaml
kubectl apply -f configmap-service-config.yaml
kubectl apply -f serviceaccount.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml
```

Confirm the pods are running:

```bash
kubectl -n automation get pods -l app=queue-keeper
kubectl -n automation logs -l app=queue-keeper --tail=50
```

---

## Expose the service

For GitHub webhook delivery you need a public HTTPS endpoint. Options:

**Ingress with TLS termination (recommended)**

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: queue-keeper
  namespace: automation
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
    - hosts:
        - webhooks.example.com
      secretName: queue-keeper-tls
  rules:
    - host: webhooks.example.com
      http:
        paths:
          - path: /webhook
            pathType: Prefix
            backend:
              service:
                name: queue-keeper
                port:
                  number: 80
```

**Azure Front Door / Application Gateway**

Terminate TLS at the gateway and forward to the internal Service on port 80. This approach also provides DDoS protection and WAF rules.

---

## Configuration changes

Queue-Keeper does not support hot-reloading. After updating a ConfigMap you must roll the Deployment:

```bash
kubectl -n automation rollout restart deployment/queue-keeper
kubectl -n automation rollout status deployment/queue-keeper
```

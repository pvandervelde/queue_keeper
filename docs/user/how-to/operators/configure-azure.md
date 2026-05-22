# Configure Azure Services

This guide provisions the Azure resources Queue-Keeper depends on: Azure Service Bus (for bot queues), Azure Blob Storage (for raw payload persistence), and Azure Key Vault (for webhook secrets). All commands use the Azure CLI.

## Prerequisites

- Azure CLI 2.50+ installed and logged in (`az login`)
- A resource group for Queue-Keeper resources
- Azure subscription with sufficient permissions

---

## 1 — Resource Group

```bash
az group create \
  --name queue-keeper-rg \
  --location eastus
```

---

## 2 — Azure Service Bus

### Create the namespace

```bash
az servicebus namespace create \
  --resource-group queue-keeper-rg \
  --name my-namespace \
  --sku Standard \
  --location eastus
```

For production use Premium tier if you need private endpoints, higher message limits, or dedicated capacity.

### Create a queue for each bot

Repeat for every bot in `bot-config.yaml`. Use `--requires-session true` when the bot has `ordered: true`.

**Ordered bot (`ordered: true`):**

```bash
az servicebus queue create \
  --resource-group queue-keeper-rg \
  --namespace-name my-namespace \
  --name queue-keeper-task-tactician \
  --requires-session true \
  --lock-duration PT5M \
  --default-message-time-to-live P14D \
  --max-delivery-count 10 \
  --enable-dead-lettering-on-message-expiration true
```

**Unordered bot (`ordered: false`):**

```bash
az servicebus queue create \
  --resource-group queue-keeper-rg \
  --namespace-name my-namespace \
  --name queue-keeper-notifications \
  --lock-duration PT5M \
  --default-message-time-to-live P14D \
  --max-delivery-count 10 \
  --enable-dead-lettering-on-message-expiration true
```

!!! warning "Session mismatch"
    If a bot has `ordered: true` in `bot-config.yaml` but the queue was created without `--requires-session true`, Azure Service Bus silently drops the `SessionId` property and delivers messages without ordering. Always create the queue first, then register the bot.

### Grant Queue-Keeper access

Using managed identity (recommended):

```bash
# Get the Queue-Keeper managed identity's principal ID
PRINCIPAL_ID=$(az identity show \
  --resource-group queue-keeper-rg \
  --name queue-keeper-identity \
  --query principalId --output tsv)

# Grant Service Bus Data Owner on the namespace
az role assignment create \
  --role "Azure Service Bus Data Owner" \
  --assignee "$PRINCIPAL_ID" \
  --scope "/subscriptions/$(az account show --query id -o tsv)/resourceGroups/queue-keeper-rg/providers/Microsoft.ServiceBus/namespaces/my-namespace"
```

---

## 3 — Azure Blob Storage

Blob Storage holds raw webhook payloads for audit and replay.

### Create the storage account

```bash
az storage account create \
  --resource-group queue-keeper-rg \
  --name queuekeeperstore \
  --sku Standard_LRS \
  --kind StorageV2 \
  --min-tls-version TLS1_2 \
  --allow-blob-public-access false
```

### Create the container

```bash
az storage container create \
  --account-name queuekeeperstore \
  --name webhook-payloads \
  --auth-mode login
```

### Grant access

```bash
STORAGE_ID=$(az storage account show \
  --resource-group queue-keeper-rg \
  --name queuekeeperstore \
  --query id --output tsv)

az role assignment create \
  --role "Storage Blob Data Contributor" \
  --assignee "$PRINCIPAL_ID" \
  --scope "$STORAGE_ID"
```

---

## 4 — Azure Key Vault

Key Vault stores the GitHub webhook secret so it is never written to disk or environment variables.

### Create the vault

```bash
az keyvault create \
  --resource-group queue-keeper-rg \
  --name my-queue-keeper-vault \
  --location eastus \
  --sku standard \
  --enable-rbac-authorization true
```

### Store the webhook secret

```bash
az keyvault secret set \
  --vault-name my-queue-keeper-vault \
  --name "github-webhook-secret" \
  --value "your-github-webhook-secret-here"
```

### Grant Queue-Keeper access

```bash
VAULT_ID=$(az keyvault show \
  --name my-queue-keeper-vault \
  --query id --output tsv)

az role assignment create \
  --role "Key Vault Secrets User" \
  --assignee "$PRINCIPAL_ID" \
  --scope "$VAULT_ID"
```

---

## 5 — Managed Identity

Create the user-assigned managed identity for Queue-Keeper:

```bash
az identity create \
  --resource-group queue-keeper-rg \
  --name queue-keeper-identity
```

Assign it to your container (Docker / AKS) using the platform-specific mechanism:

- **AKS workload identity**: Annotate the pod's `ServiceAccount` with the client ID (see [Deploy on Kubernetes](deploy-kubernetes.md))
- **Azure Container Apps**: Use `--user-assigned` on `az containerapp identity assign`
- **Azure Container Instances**: Use `--assign-identity` on `az container create`

---

## `service.yaml` snippet

Once all resources are provisioned, reference them in `service.yaml`:

```yaml
key_vault:
  vault_url: "https://my-queue-keeper-vault.vault.azure.net"

queue:
  azure_service_bus:
    namespace_url: "https://my-namespace.servicebus.windows.net"
    use_managed_identity: true
```

Blob Storage is configured separately in the `webhooks` section when raw payload persistence is enabled.

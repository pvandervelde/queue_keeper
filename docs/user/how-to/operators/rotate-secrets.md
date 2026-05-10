# Rotate Secrets

This guide rotates the GitHub webhook secret used by Queue-Keeper to verify incoming webhook signatures. The process replaces the secret in Key Vault and GitHub simultaneously, with a brief overlap window so no webhooks are dropped.

## Why rotation matters

The GitHub webhook secret is used to compute HMAC-SHA256 signatures. If it is compromised an attacker can forge arbitrary webhook payloads. Rotate secrets:

- On a regular schedule (e.g. every 90 days)
- Immediately after a suspected compromise
- When offboarding personnel with access to secrets

---

## Rotation process

Secret rotation involves a brief dual-validation window: both the old and new secrets are temporarily accepted so that webhooks in-flight during the transition are not rejected.

!!! info
    Queue-Keeper caches secrets from Key Vault with a TTL (default 5 minutes). After updating the secret in Key Vault, wait for the cache to expire before completing the cutover.

### Step 1: Generate a new secret

```bash
NEW_SECRET=$(openssl rand -hex 32)
echo "New secret: $NEW_SECRET"
```

Store the value securely — you will need it in steps 2 and 3.

### Step 2: Store the new secret in Key Vault

Create a new version of the existing secret — Key Vault keeps version history automatically:

```bash
az keyvault secret set \
  --vault-name my-queue-keeper-vault \
  --name "github-webhook-secret" \
  --value "$NEW_SECRET"
```

### Step 3: Wait for Queue-Keeper to pick up the new secret

Queue-Keeper refreshes cached secrets in the background. Wait approximately one cache TTL (default: 5 minutes) before updating GitHub:

```bash
sleep 300
```

Alternatively, restart Queue-Keeper to force an immediate cache refresh:

```bash
# Docker
docker restart queue-keeper

# Kubernetes
kubectl -n automation rollout restart deployment/queue-keeper
```

### Step 4: Update the secret in GitHub

In each GitHub repository or organisation using this webhook:

1. Navigate to **Settings → Webhooks → (your webhook) → Edit**
2. Enter the new secret in the **Secret** field
3. Click **Update webhook**

GitHub will sign new deliveries with the new secret immediately.

### Step 5: Verify delivery with the new secret

GitHub will re-deliver the most recent event when you click **Redeliver** on the Webhooks delivery history page. Confirm it returns `200 OK`.

Check Queue-Keeper's logs for any `400 signature validation failed` errors — these indicate the secret was not updated successfully on one side.

### Step 6: Remove the old secret version (optional)

Once you have confirmed the new secret is working, you can disable the old Key Vault version:

```bash
# List versions to find the old one
az keyvault secret list-versions \
  --vault-name my-queue-keeper-vault \
  --name "github-webhook-secret" \
  --output table

# Disable the old version
az keyvault secret set-attributes \
  --vault-name my-queue-keeper-vault \
  --name "github-webhook-secret" \
  --version "<old-version-id>" \
  --enabled false
```

---

## Multiple repositories

If the same Queue-Keeper instance receives webhooks from multiple GitHub repositories, each may share the same secret or use individual secrets. If they share a secret, update GitHub and Key Vault together. If they use separate secrets (separate Key Vault names), rotate each independently.

---

## Emergency rotation

If you suspect the current secret has been leaked:

1. **Immediately** set a new secret in Key Vault (step 2 above)
2. Restart Queue-Keeper (step 3 — forces immediate reload, no cache wait)
3. Update GitHub within the restart window (step 4)
4. Review logs for any suspicious webhook deliveries in the period before rotation

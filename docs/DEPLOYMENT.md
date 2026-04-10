# Hearth Home Cluster Deployment

This document covers production-grade deployment of the Hearth Home Cluster
Helm chart. For the local-laptop demo flow see `docs/DEMO.md`.

## Contents
- [Deployment profiles](#deployment-profiles)
- [Quickstart (dev/air-gapped)](#quickstart-devair-gapped)
- [TLS certificate management](#tls-certificate-management)
- [Secret management](#secret-management)
- [Capability toggles](#capability-toggles)
- [Upgrades](#upgrades)
- [Troubleshooting](#troubleshooting)

## Deployment profiles

Hearth supports three progressively more enterprise-friendly deployment
shapes. Pick the one that matches your operational maturity — you can
always grow from one to the next without replatforming.

| Profile         | TLS certs                    | Secrets                 | Best for                                   |
|-----------------|------------------------------|-------------------------|--------------------------------------------|
| **Quickstart**  | `genSelfSignedCert` (Helm)   | In-values / generated   | Dev, demos, air-gapped single-node         |
| **Production**  | cert-manager                 | k8s Secrets + RBAC      | Most self-hosted prod deployments          |
| **Enterprise**  | cert-manager / ACME          | External Secrets + Vault/AWS/GCP/etc. | Orgs with existing secret infra  |

The chart doesn't bundle a key vault or secret store — bringing in your own
is safer and integrates with the policies you already have.

## Quickstart (dev/air-gapped)

The simplest deployment. Zero external dependencies.

```bash
helm install hearth ./chart/hearth-home \
  --create-namespace \
  --namespace hearth-home \
  --set capabilities.identity=true \
  --set capabilities.mesh=true
```

What this gets you:
- Kanidm's TLS cert is generated at install time via Helm's
  `genSelfSignedCert` and preserved across upgrades via `lookup`.
- Self-signed certs carry a 10-year validity — no renewal story.
- All component secrets are generated in-cluster or come from values.

Good enough for laptops and demos; not recommended beyond that.

## TLS certificate management

### Option A: cert-manager + self-signed internal CA (recommended for prod)

This is the cleanest path for most deployments. cert-manager handles cert
lifecycle (issuance, renewal, rotation) and works entirely offline via a
self-signed ClusterIssuer.

**Prerequisites:** install cert-manager in the cluster.
```bash
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.15.0/cert-manager.yaml
```

**Enable in values:**
```yaml
certManager:
  enabled: true
  issuer:
    type: selfSigned
```

That's it. The chart creates:
- A `ClusterIssuer` named `{release}-issuer` (self-signed root)
- A `Certificate` resource for Kanidm (`{release}-kanidm-tls`)

The resulting `Secret` has the same name the Kanidm StatefulSet and the
Synapse init container already mount. Certs renew automatically.

### Option B: cert-manager + Let's Encrypt (public hostnames)

If your Hearth endpoints are reachable on the public internet at a real
domain, use ACME to get publicly-trusted certs with no manual CA wrangling.

```yaml
certManager:
  enabled: true
  issuer:
    type: acme
    acme:
      server: https://acme-v02.api.letsencrypt.org/directory
      email: ops@example.com
      solver: http01
      ingressClass: nginx
```

For DNS-01 (wildcard certs, private clusters):
```yaml
certManager:
  enabled: true
  issuer:
    type: acme
    acme:
      email: ops@example.com
      solver: dns01
      dns01:
        cloudflare:
          email: ops@example.com
          apiTokenSecretRef:
            name: cloudflare-api-token
            key: api-token
```

You own the DNS provider creds separately (typically via External Secrets —
see below).

### Option C: BYO issuer (existing ClusterIssuer)

If your cluster already has a ClusterIssuer managed elsewhere (platform
team, another chart, GitOps), reference it directly:

```yaml
certManager:
  enabled: true
  issuer:
    type: existing
    existing:
      name: company-internal-ca
      kind: ClusterIssuer
```

### Option D: BYO secret (cert provisioned out-of-band)

If you're managing certs entirely outside k8s — e.g., a PKI team hands you
a `tls.crt`/`tls.key` pair — create the Secret yourself and point Hearth
at it:

```bash
kubectl create secret tls my-kanidm-tls \
  --cert=kanidm.crt --key=kanidm.key \
  -n hearth-home
```

```yaml
certManager:
  enabled: false  # skip cert-manager entirely
kanidm:
  tls:
    existingSecret: my-kanidm-tls
```

This composes with any of the other secret-management options — External
Secrets, Sealed Secrets, etc. — as long as the final artifact is a
`kubernetes.io/tls` Secret with the expected keys.

## Secret management

Hearth uses standard k8s Secrets for everything: OIDC client secrets, DB
passwords, admin tokens. Every component exposes an `existingSecret`
field so you can BYO. **The chart does not host a secret store.** Pick
one of the following based on your policy.

### Generated in-values (quickstart only)

The default path. Secrets are either generated by the chart or pulled
from inline values. Fine for dev, weak for anything with auditors.

### External Secrets Operator (recommended for prod)

[External Secrets Operator](https://external-secrets.io/) bridges k8s
Secrets to an external store: HashiCorp Vault, AWS Secrets Manager, GCP
Secret Manager, Azure Key Vault, 1Password, Infisical, etc.

**Pattern:**

1. Install ESO in the cluster (one-time).
2. Store your secrets in your existing vault.
3. For each `existingSecret` field in Hearth's values, create an
   `ExternalSecret` CR that pulls from the vault and materializes the k8s
   Secret. Example for the Synapse OIDC client secret:

    ```yaml
    apiVersion: external-secrets.io/v1beta1
    kind: ExternalSecret
    metadata:
      name: hearth-matrix-oidc
      namespace: hearth-home
    spec:
      refreshInterval: 1h
      secretStoreRef:
        name: vault-backend
        kind: ClusterSecretStore
      target:
        name: hearth-matrix-oidc  # must match existingSecret below
      data:
        - secretKey: matrix-oidc-secret
          remoteRef:
            key: secret/hearth/matrix-oidc
            property: value
    ```

4. Reference the materialized Secret in Hearth values:

    ```yaml
    synapse:
      oidc:
        existingSecret: hearth-matrix-oidc
        existingSecretKey: matrix-oidc-secret
    ```

Repeat for Nextcloud OIDC, DB passwords, Stalwart creds, etc. This is
invisible to Hearth — it just sees a Secret. Your vault remains the
source of truth.

### Sealed Secrets (GitOps-friendly)

If your deployment model is "everything lives in git", use
[Sealed Secrets](https://github.com/bitnami-labs/sealed-secrets). You
commit encrypted Secrets to the repo; the controller decrypts them
in-cluster. No runtime vault dependency.

Same pattern as ESO: materialize a k8s Secret, then reference it via
`existingSecret`.

## Capability toggles

The chart's cost/complexity scales with what you enable. Start small.

```yaml
capabilities:
  identity: true         # Kanidm — required by everything else that does SSO
  mesh: true             # Headscale — WireGuard coordination
  builds: true           # Build worker for Nix closures
  chat: false            # Synapse/Element — off by default
  cloud: false           # Nextcloud — off by default
  mail: false            # Stalwart — off by default
  observability: false   # Grafana/Loki/Prometheus subcharts
```

Enable them incrementally. Each capability has its own `existingSecret`
escape hatches so you can provision secrets from whichever store you
standardized on above.

## Upgrades

- Chart upgrades are in-place. The `lookup` pattern in the TLS secret
  template means a self-signed cert doesn't regenerate on upgrade.
- When migrating from the self-signed path to cert-manager:
    1. Install cert-manager.
    2. Set `certManager.enabled: true` in your values.
    3. `helm upgrade` — the chart will create a Certificate resource and
       cert-manager will overwrite the existing Secret on next
       reconciliation.
    4. Bounce any pods that have the old cert cached (`kubectl rollout
       restart deployment -n hearth-home`).
- Kanidm data is in a PVC; Synapse data is in PVCs. Back these up before
  non-trivial upgrades.

## Troubleshooting

### Synapse pod crash-loops with "RequestTimedOutError" on startup

Synapse's OIDC provider fetches metadata from Kanidm at startup. If the
Kanidm TLS cert isn't in Synapse's trust store, the TLS handshake fails
and Twisted reports it as a timeout (misleading). The chart handles this
automatically via an init container that installs the Kanidm cert —
verify that:

- `capabilities.identity` and `synapse.oidc.enabled` are both true
- The `{release}-kanidm-tls` Secret exists and has `tls.crt`
- The Synapse pod logs show "Mapped kanidm.hearth.local -> <ip>" from
  the `write-kanidm-hosts` init container

### cert-manager ClusterIssuer is Ready=False

```bash
kubectl describe clusterissuer {release}-issuer
```

For ACME issuers, check that the HTTP-01 solver ingress is reachable
from the internet (Let's Encrypt can't reach private clusters with
HTTP-01). Switch to DNS-01 for private deployments.

### External Secret not materializing

```bash
kubectl describe externalsecret <name> -n hearth-home
kubectl describe clustersecretstore vault-backend
```

Common causes: ESO service account lacks the IAM/Vault policy, the
vault path is wrong, or the target secret name collides with an
existing Secret not owned by ESO.

### Kanidm cert regenerated after upgrade

If you're on the self-signed path and the cert changed unexpectedly,
check that the `{release}-kanidm-tls` Secret wasn't manually deleted.
The `lookup` template preserves the cert across upgrades only if the
Secret still exists at upgrade time.

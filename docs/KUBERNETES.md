# Kubernetes Deployment and GitOps with FluxCD

This guide explains how to deploy Oxifed to a Kubernetes cluster and manage it using GitOps with FluxCD.

## Architecture

In Kubernetes, Oxifed consists of:
- `domainservd`: The main API service.
- `publisherd`: The background activity publishing service.
- `oxifed-operator`: A custom controller that manages `Domain` resources.
- `mongodb`: The database (deployed as a single-node for development).
- `lavinmq`: The message queue.

## Prerequisites

- A Kubernetes cluster.
- `kubectl` configured to point to your cluster.
- FluxCD installed in your cluster.

## Deployment Steps

### 1. Using Pre-built Images (Recommended)

Oxifed images are automatically built and published to GitHub Container Registry (GHCR) for every release and push to the `main` branch.

The default manifests in `k8s/base` already point to:
- `ghcr.io/oxifed/oxifed/domainservd:latest`
- `ghcr.io/oxifed/oxifed/publisherd:latest`
- `ghcr.io/oxifed/oxifed/oxifed-operator:latest`

### 2. Custom Build and Push (Optional)

If you want to use your own images:

1. Build and push the Docker images:
   ```bash
   docker build -t youruser/domainservd:latest -f docker/domainservd/Dockerfile .
   docker build -t youruser/publisherd:latest -f docker/publisherd/Dockerfile .
   docker build -t youruser/oxifed-operator:latest -f docker/oxifed-operator/Dockerfile .

   docker push youruser/domainservd:latest
   docker push youruser/publisherd:latest
   docker push youruser/oxifed-operator:latest
   ```

2. Update the image names in `k8s/base/*.yaml` or use Kustomize `images` transformer in your overlay.

### 3. Configure FluxCD

The `flux/` directory contains the manifests to set up GitOps.

1.  Edit `flux/clusters/dev/oxifed-sync.yaml` and update the `url` to point to your Git repository.
2.  Apply the Flux manifests:

```bash
kubectl apply -f flux/clusters/dev/oxifed-sync.yaml
```

Flux will now monitor your repository and apply the manifests in `k8s/overlays/dev`.

### 3. Manual Deployment (Alternative)

If you don't want to use Flux, you can apply the Kustomize overlay directly:

```bash
kubectl apply -k k8s/overlays/dev
```

## Managing Domains with CRDs

Oxifed includes a Kubernetes Operator that allows you to manage federation domains using Custom Resource Definitions (CRDs).

### Registering a New Domain

To register a domain, create a `Domain` resource:

```yaml
apiVersion: oxifed.io/v1alpha1
kind: Domain
metadata:
  name: my-cool-domain
spec:
  hostname: cool.example.com
  description: "A very cool community"
  adminEmail: admin@cool.example.com
```

Apply it with `kubectl`:

```bash
kubectl apply -f my-domain.yaml
```

The `oxifed-operator` will pick up the new resource and initialize the domain in the system.

> **Note:** The operator currently generates mock key material (see [KNOWN_ISSUES.md](KNOWN_ISSUES.md)). Keys stored in Kubernetes Secrets are not real cryptographic keys.

## Troubleshooting

### Check Pod Status
```bash
kubectl get pods -n oxifed-dev
```

### Check Operator Logs
```bash
kubectl logs -l app=oxifed-operator -n oxifed-dev
```

### Check Domain Status
```bash
kubectl get domains -n oxifed-dev
```

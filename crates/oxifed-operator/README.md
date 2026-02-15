# oxifed-operator -- Kubernetes Operator

Kubernetes operator that manages `Domain` Custom Resources (CRD version: v1alpha1).

## What It Does

1. Watches for `Domain` CRD create/update/delete events
2. Generates cryptographic key pairs for each domain
3. Stores key material in Kubernetes Secrets
4. Syncs domain configuration to MongoDB

## Known Issue: Mock Keys

The operator uses the `oxifed` crate's PKI module for key generation, which currently returns mock PEM strings (e.g., `MOCK_RSA_PUBLIC_KEY_2048`). The keys stored in Kubernetes Secrets are **not real cryptographic keys** and cannot be used for HTTP signature operations.

This will be resolved when real key generation is implemented in `src/pki.rs`.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MONGODB_URI` | `mongodb://root:password@localhost:27017` | MongoDB connection |
| `MONGODB_DBNAME` | `domainservd` | Database name |

## Running

The operator is intended to run inside a Kubernetes cluster. See [docs/KUBERNETES.md](../../docs/KUBERNETES.md) for deployment instructions.

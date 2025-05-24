# CI/CD Pipeline Documentation

This document describes the Continuous Integration and Continuous Deployment (CI/CD) pipeline for the Oxifed ActivityPub platform.

## Overview

The CI/CD pipeline is implemented using GitHub Actions and consists of two main workflows:

1. **Pull Request Checks** (`pr.yml`) - Fast feedback for pull requests
2. **CI/CD Pipeline** (`ci.yml`) - Full testing and Docker image publishing

## Workflows

### Pull Request Checks (`pr.yml`)

Triggered on all pull requests to `main` and `develop` branches.

**Jobs:**
- **Test Suite**: Runs formatting checks, clippy linting, builds, and tests
- **Security Audit**: Performs security vulnerability scanning using `cargo audit`

**Features:**
- Fast feedback cycle optimized for development
- Comprehensive caching to reduce build times
- Parallel job execution

### CI/CD Pipeline (`ci.yml`)

Triggered on pushes to `main` and `develop` branches, and on pull requests.

**Jobs:**
- **Test Suite**: Same as PR workflow
- **Security Audit**: Same as PR workflow  
- **Build and Publish**: Builds and publishes Docker images (only on push to main/develop)
- **Dependency Review**: Reviews dependencies for security issues (only on PRs)

## Docker Images

The pipeline builds and publishes Docker images for two services:

### domainservd
- **Purpose**: Domain Service Daemon with WebFinger (RFC 7033) support
- **Registry**: `ghcr.io/<repository>/domainservd`
- **Dockerfile**: `docker/domainservd/Dockerfile`
- **Port**: 8080

### publisherd  
- **Purpose**: Publisher daemon for ActivityPub message processing
- **Registry**: `ghcr.io/<repository>/publisherd`
- **Dockerfile**: `docker/publisherd/Dockerfile`

## Image Tagging Strategy

Docker images are tagged with:
- `latest` - Latest stable version from main branch
- `<branch-name>` - Branch-specific builds
- `<branch-name>-<sha>` - Specific commit builds
- `pr-<number>` - Pull request builds

## Container Registry

Images are published to GitHub Container Registry (ghcr.io) with the following benefits:
- Integrated with GitHub repository permissions
- Free for public repositories
- Automatic cleanup policies available
- Built-in vulnerability scanning

## Security Features

### Code Security
- **cargo audit**: Scans for known security vulnerabilities in dependencies
- **cargo clippy**: Lints for common mistakes and security issues
- **Dependency Review**: GitHub's dependency review action for PRs

### Container Security
- Multi-stage Docker builds to minimize attack surface
- Non-root user execution in containers
- Minimal base images (Debian slim)
- Only essential runtime dependencies included

## Caching Strategy

The pipeline uses multiple caching layers:

1. **Cargo Registry Cache**: Caches downloaded crates
2. **Cargo Index Cache**: Caches crates.io index
3. **Build Cache**: Caches compiled artifacts
4. **Docker Layer Cache**: Caches Docker build layers using GitHub Actions cache

## Prerequisites

### Repository Setup
1. Enable GitHub Actions in repository settings
2. Ensure `GITHUB_TOKEN` has package write permissions
3. Enable GitHub Container Registry

### Branch Protection
Recommended branch protection rules for `main`:
- Require pull request reviews
- Require status checks to pass
- Require up-to-date branches
- Include administrators in restrictions

## Local Development

### Running Tests Locally
```bash
# Format check
cargo fmt --all -- --check

# Linting
cargo clippy --all-targets --all-features -- -D warnings

# Tests
cargo test --all-features --workspace

# Doc tests
cargo test --doc --workspace

# Security audit
cargo audit
```

### Building Docker Images Locally
```bash
# Build domainservd
docker build -f docker/domainservd/Dockerfile -t domainservd .

# Build publisherd  
docker build -f docker/publisherd/Dockerfile -t publisherd .
```

## Troubleshooting

### Common Issues

**Build Failures:**
- Check Rust version compatibility (requires 1.70+)
- Verify all dependencies are available
- Check for formatting issues with `cargo fmt`

**Docker Build Failures:**
- Ensure all required binaries exist in target/release/
- Check Dockerfile paths and context
- Verify base image availability

**Cache Issues:**
- Cache keys are based on Cargo.lock hash
- Delete and recreate caches if corrupted
- Check cache size limits (GitHub has 10GB limit)

### Debug Steps
1. Check workflow logs in Actions tab
2. Verify environment variables and secrets
3. Test builds locally with same Rust version
4. Check dependency versions and compatibility

## Monitoring

### Key Metrics to Monitor
- Build success rate
- Build duration trends  
- Test execution time
- Docker image sizes
- Security scan results

### Alerts
Consider setting up notifications for:
- Failed builds on main branch
- Security vulnerabilities detected
- Long-running builds (timeout issues)

## Maintenance

### Regular Tasks
- Update Rust version in workflows (monthly)
- Review and update dependency versions
- Clean up old container images
- Monitor cache usage and cleanup old caches
- Review security audit results

### Updating Workflows
When modifying workflows:
1. Test changes in feature branch first
2. Monitor initial runs carefully
3. Update documentation accordingly
4. Consider backward compatibility
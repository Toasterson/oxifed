# Development Scripts

This directory contains shell scripts for local development tasks that mirror the CI/CD pipeline checks and provide convenient development workflows.

## Quick Start

```bash
# Initial setup (first time)
./scripts/dev.sh setup

# Daily development workflow
./scripts/dev.sh test      # Run tests before committing
./scripts/dev.sh format    # Fix formatting issues
./scripts/dev.sh start     # Start development environment
```

## Available Scripts

### 🚀 Main Development Helper (`dev.sh`)
Primary script that orchestrates all development tasks.

```bash
./scripts/dev.sh <command> [options]
```

**Common Commands:**
- `setup` - Set up the complete development environment
- `test` - Run all tests locally (mirrors CI checks)
- `format` - Auto-format code and apply clippy fixes
- `start` - Start all development services
- `stop` - Stop all development services
- `status` - Show status of all services
- `logs [service]` - View logs for all or specific service
- `clean` - Clean up development environment

**Development Commands:**
- `run <service>` - Run a specific service locally (domainservd, publisherd, oxiadm)
- `shell <service>` - Open shell in service container
- `db` - Connect to MongoDB shell
- `mq` - Open RabbitMQ management interface

### 🧪 Test Suite (`test-local.sh`)
Runs the complete test suite that mirrors the CI/CD pipeline:
- Code formatting checks
- Clippy linting with strict warnings
- Full test suite execution
- Documentation tests
- Security vulnerability scanning
- Binary build verification

### 🐳 Docker Build (`docker-build.sh`)
Builds and tests Docker images locally:
- Builds both `domainservd` and `publisherd` images
- Tests container startup
- Reports image sizes
- Validates Docker configuration

### ⚙️ Environment Setup (`dev-setup.sh`)
Sets up the complete development environment:
- Validates all dependencies
- Starts infrastructure services (MongoDB, LavinMQ)
- Builds and starts application services
- Performs health checks
- Provides service URLs and useful commands

### ✨ Format & Fix (`format-fix.sh`)
Automatically fixes common code issues:
- Applies rustfmt formatting
- Runs clippy auto-fixes
- Optionally updates dependencies (`--update`)
- Verifies fixes don't break compilation

### 🧹 Cleanup (`cleanup.sh`)
Cleans up development environment with granular options:
- `--docker` - Clean Docker containers, images, and volumes
- `--cargo` - Clean Cargo build artifacts and cache
- `--logs` - Clean log files and temporary files
- `--all` - Clean everything (default)

## Prerequisites

All scripts require:
- **Rust 1.70+** with cargo
- **Docker** and **Docker Compose**
- **curl** (for health checks)

Scripts will check for these dependencies and provide installation guidance if missing.

## Development Workflow

### 1. Initial Setup
```bash
# Clone the repository
git clone <repository-url>
cd oxifed

# Set up development environment
./scripts/dev.sh setup
```

### 2. Daily Development
```bash
# Start development environment
./scripts/dev.sh start

# Make your changes...

# Test your changes (before committing)
./scripts/dev.sh test

# Fix any formatting issues
./scripts/dev.sh format

# Check service status
./scripts/dev.sh status

# View logs if needed
./scripts/dev.sh logs domainservd
```

### 3. Testing Changes
```bash
# Test specific service locally
./scripts/dev.sh run domainservd

# Build and test Docker images
./scripts/dev.sh build

# Restart services after changes
./scripts/dev.sh restart domainservd
```

### 4. Cleanup
```bash
# Clean up when done
./scripts/dev.sh clean
```

## Service URLs

When the development environment is running:

- **Domain Service**: http://localhost:8080
- **MongoDB**: mongodb://localhost:27017 (root/password)
- **LavinMQ Management**: http://localhost:15672
- **LavinMQ AMQP**: amqp://localhost:5672

## Troubleshooting

### Common Issues

**Permission Denied:**
```bash
chmod +x scripts/*.sh
```

**Docker Not Running:**
```bash
# Start Docker daemon
sudo systemctl start docker  # Linux
# or start Docker Desktop    # macOS/Windows
```

**Port Conflicts:**
```bash
# Check what's using ports
sudo lsof -i :8080
sudo lsof -i :27017
sudo lsof -i :5672
```

**Build Failures:**
```bash
# Clean and rebuild
./scripts/dev.sh clean --all
./scripts/dev.sh setup
```

### Getting Help

Each script provides help with the `--help` flag:
```bash
./scripts/dev.sh --help
./scripts/cleanup.sh --help
```

## Script Features

### 🎨 Colored Output
All scripts use colored output for better readability:
- 🔵 Blue: Step indicators
- ✅ Green: Success messages
- ⚠️ Yellow: Warnings
- ❌ Red: Errors

### 🔄 Caching
Scripts are optimized for speed with caching:
- Cargo registry and build caches
- Docker layer caching
- Dependency caching

### 🛡️ Safety
Scripts include safety features:
- Project root directory validation
- Dependency checking
- Graceful error handling
- Confirmation prompts for destructive operations

### 📊 Progress Tracking
Scripts provide clear progress indication:
- Step-by-step progress
- Health checks with retries
- Summary of completed actions
- Next steps guidance

## Integration with CI/CD

These scripts mirror the GitHub Actions workflows:
- `test-local.sh` matches the CI test pipeline
- `docker-build.sh` matches the Docker build process
- Same Rust version and dependency requirements
- Same security and formatting checks

This ensures local development matches the CI/CD environment exactly.
#!/bin/bash

# Local Development Test Script
# Runs the same checks as CI/CD pipeline locally

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_step() {
    echo -e "${BLUE}==> $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
}

# Check if we're in the project root
if [ ! -f "Cargo.toml" ]; then
    print_error "This script must be run from the project root directory"
    exit 1
fi

print_step "Starting local development tests..."

# Check Rust installation
print_step "Checking Rust installation..."
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi

RUST_VERSION=$(rustc --version)
print_success "Rust found: $RUST_VERSION"

# Check formatting
print_step "Checking code formatting..."
if cargo fmt --all -- --check; then
    print_success "Code formatting is correct"
else
    print_error "Code formatting issues found. Run 'cargo fmt' to fix them."
    exit 1
fi

# Run clippy
print_step "Running clippy linter..."
if cargo clippy --all-targets --all-features -- -D warnings; then
    print_success "Clippy checks passed"
else
    print_error "Clippy found issues. Please fix the warnings above."
    exit 1
fi

# Build all targets
print_step "Building all targets..."
if cargo build --all-targets --all-features; then
    print_success "Build completed successfully"
else
    print_error "Build failed"
    exit 1
fi

# Run tests
print_step "Running test suite..."
if cargo test --all-features --workspace; then
    print_success "All tests passed"
else
    print_error "Some tests failed"
    exit 1
fi

# Run doc tests
print_step "Running documentation tests..."
if cargo test --doc --workspace; then
    print_success "Documentation tests passed"
else
    print_error "Documentation tests failed"
    exit 1
fi

# Check for security vulnerabilities (optional)
print_step "Checking for security vulnerabilities..."
if command -v cargo-audit &> /dev/null; then
    if cargo audit; then
        print_success "No security vulnerabilities found"
    else
        print_warning "Security vulnerabilities found. Please review the output above."
    fi
else
    print_warning "cargo-audit not installed. Install with: cargo install cargo-audit"
fi

# Check binary builds
print_step "Verifying service binaries..."
BINARIES=("domainservd" "publisherd" "oxiadm")
for binary in "${BINARIES[@]}"; do
    if [ -f "target/debug/$binary" ] || [ -f "target/release/$binary" ]; then
        print_success "$binary binary built successfully"
    else
        print_warning "$binary binary not found in target directory"
    fi
done

print_step "Local development tests completed successfully!"
print_success "Your code is ready for CI/CD pipeline"

echo ""
echo "Next steps:"
echo "  - Run 'scripts/docker-build.sh' to test Docker builds"
echo "  - Run 'scripts/dev-setup.sh' to start development environment"
echo "  - Run 'cargo run --bin <service>' to test individual services"
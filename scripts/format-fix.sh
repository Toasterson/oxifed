#!/bin/bash

# Quick Format and Fix Script
# Automatically fixes common formatting and linting issues

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

print_step "Running automatic formatting and fixes..."

# Fix code formatting
print_step "Formatting code with rustfmt..."
if cargo fmt --all; then
    print_success "Code formatting applied"
else
    print_error "Failed to format code"
    exit 1
fi

# Fix clippy issues that can be auto-fixed
print_step "Applying clippy automatic fixes..."
if cargo clippy --all-targets --all-features --fix --allow-dirty --allow-staged; then
    print_success "Clippy automatic fixes applied"
else
    print_warning "Some clippy issues require manual intervention"
fi

# Update dependencies (optional, only if --update flag is passed)
if [ "$1" = "--update" ]; then
    print_step "Updating dependencies..."
    cargo update
    print_success "Dependencies updated"
fi

# Run a quick test to make sure fixes didn't break anything
print_step "Running quick verification..."
if cargo check --all-targets --all-features; then
    print_success "Quick verification passed"
else
    print_error "Fixes may have introduced compilation errors"
    exit 1
fi

print_success "Formatting and fixes completed!"

echo ""
echo "What was done:"
echo "  ✅ Code formatted with rustfmt"
echo "  ✅ Clippy auto-fixes applied"
if [ "$1" = "--update" ]; then
    echo "  ✅ Dependencies updated"
fi
echo "  ✅ Quick compilation check passed"

echo ""
echo "Next steps:"
echo "  - Review changes with 'git diff'"
echo "  - Run 'scripts/test-local.sh' for full testing"
echo "  - Commit your changes when ready"
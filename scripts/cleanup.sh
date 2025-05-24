#!/bin/bash

# Development Environment Cleanup Script
# Cleans up Docker containers, images, and build artifacts

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

print_step "Cleaning up Oxifed development environment..."

# Parse command line arguments
CLEAN_ALL=false
CLEAN_DOCKER=false
CLEAN_CARGO=false
CLEAN_LOGS=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --all)
            CLEAN_ALL=true
            shift
            ;;
        --docker)
            CLEAN_DOCKER=true
            shift
            ;;
        --cargo)
            CLEAN_CARGO=true
            shift
            ;;
        --logs)
            CLEAN_LOGS=true
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --all     Clean everything (Docker + Cargo + logs)"
            echo "  --docker  Clean Docker containers, images, and volumes"
            echo "  --cargo   Clean Cargo build artifacts"
            echo "  --logs    Clean log files"
            echo "  -h, --help Show this help message"
            exit 0
            ;;
        *)
            print_error "Unknown option: $1"
            echo "Use --help for usage information"
            exit 1
            ;;
    esac
done

# If no specific options, clean everything
if [ "$CLEAN_ALL" = false ] && [ "$CLEAN_DOCKER" = false ] && [ "$CLEAN_CARGO" = false ] && [ "$CLEAN_LOGS" = false ]; then
    CLEAN_ALL=true
fi

# Stop and remove containers
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_DOCKER" = true ]; then
    print_step "Stopping and removing Docker containers..."
    
    if docker-compose ps -q | grep -q .; then
        docker-compose down -v
        print_success "Docker containers stopped and removed"
    else
        print_success "No running containers to stop"
    fi
    
    # Remove project-specific images
    print_step "Removing project Docker images..."
    PROJECT_IMAGES=$(docker images --filter=reference="oxifed-*" -q)
    if [ -n "$PROJECT_IMAGES" ]; then
        docker rmi $PROJECT_IMAGES
        print_success "Project Docker images removed"
    else
        print_success "No project Docker images to remove"
    fi
    
    # Clean up dangling images and containers
    print_step "Cleaning up dangling Docker resources..."
    docker system prune -f
    print_success "Dangling Docker resources cleaned"
    
    # Remove volumes (optional - be careful as this removes data)
    read -p "Remove Docker volumes (this will delete all data)? [y/N]: " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        print_step "Removing Docker volumes..."
        docker volume prune -f
        OXIFED_VOLUMES=$(docker volume ls --filter=name=oxifed -q)
        if [ -n "$OXIFED_VOLUMES" ]; then
            docker volume rm $OXIFED_VOLUMES
        fi
        print_success "Docker volumes removed"
    else
        print_success "Docker volumes preserved"
    fi
fi

# Clean Cargo build artifacts
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_CARGO" = true ]; then
    print_step "Cleaning Cargo build artifacts..."
    
    if [ -d "target" ]; then
        cargo clean
        print_success "Cargo build artifacts cleaned"
    else
        print_success "No Cargo build artifacts to clean"
    fi
    
    # Clean cargo cache (optional)
    read -p "Clean Cargo cache (will require re-downloading dependencies)? [y/N]: " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if [ -d "$HOME/.cargo/registry" ]; then
            rm -rf "$HOME/.cargo/registry"
            print_success "Cargo registry cache cleaned"
        fi
        if [ -d "$HOME/.cargo/git" ]; then
            rm -rf "$HOME/.cargo/git"
            print_success "Cargo git cache cleaned"
        fi
    else
        print_success "Cargo cache preserved"
    fi
fi

# Clean log files
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_LOGS" = true ]; then
    print_step "Cleaning log files..."
    
    # Find and remove common log files
    find . -name "*.log" -type f -delete 2>/dev/null || true
    find . -name "*.log.*" -type f -delete 2>/dev/null || true
    
    # Clean temporary files
    find . -name "*.tmp" -type f -delete 2>/dev/null || true
    find . -name ".DS_Store" -type f -delete 2>/dev/null || true
    
    print_success "Log files and temporary files cleaned"
fi

# Clean IDE and editor artifacts
print_step "Cleaning IDE and editor artifacts..."
rm -rf .idea/.runConfigurations 2>/dev/null || true
rm -rf .vscode/.browse.* 2>/dev/null || true
find . -name "*.swp" -type f -delete 2>/dev/null || true
find . -name "*.swo" -type f -delete 2>/dev/null || true
print_success "IDE artifacts cleaned"

# Show disk space saved
print_step "Cleanup summary:"
df -h . | tail -1 | awk '{print "Available disk space: " $4}'

print_success "Cleanup completed!"

echo ""
echo "What was cleaned:"
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_DOCKER" = true ]; then
    echo "  🐳 Docker containers and images"
fi
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_CARGO" = true ]; then
    echo "  🦀 Cargo build artifacts"
fi
if [ "$CLEAN_ALL" = true ] || [ "$CLEAN_LOGS" = true ]; then
    echo "  📄 Log files and temporary files"
fi
echo "  🧹 IDE and editor artifacts"

echo ""
echo "To rebuild the development environment:"
echo "  scripts/dev-setup.sh"
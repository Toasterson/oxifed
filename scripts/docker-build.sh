#!/bin/bash

# Docker Build Script for Local Development
# Builds and tests Docker images locally

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

# Check if Docker is installed and running
print_step "Checking Docker installation..."
if ! command -v docker &> /dev/null; then
    print_error "Docker not found. Please install Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

if ! docker info &> /dev/null; then
    print_error "Docker daemon is not running. Please start Docker."
    exit 1
fi

print_success "Docker is available and running"

# Services to build
SERVICES=("domainservd" "publisherd")

# Build each service
for service in "${SERVICES[@]}"; do
    print_step "Building Docker image for $service..."
    
    if docker build -f "docker/$service/Dockerfile" -t "oxifed-$service:local" .; then
        print_success "$service image built successfully"
    else
        print_error "Failed to build $service image"
        exit 1
    fi
    
    # Check image size
    IMAGE_SIZE=$(docker images --format "table {{.Size}}" "oxifed-$service:local" | tail -n 1)
    print_success "$service image size: $IMAGE_SIZE"
done

# Test basic functionality
print_step "Testing Docker images..."

for service in "${SERVICES[@]}"; do
    print_step "Testing $service container startup..."
    
    # Start container and check if it starts without errors
    CONTAINER_ID=$(docker run -d "oxifed-$service:local")
    
    # Wait a moment for startup
    sleep 2
    
    # Check if container is still running
    if docker ps -q --filter id="$CONTAINER_ID" | grep -q .; then
        print_success "$service container started successfully"
        docker stop "$CONTAINER_ID" > /dev/null
        docker rm "$CONTAINER_ID" > /dev/null
    else
        print_error "$service container failed to start"
        docker logs "$CONTAINER_ID"
        docker rm "$CONTAINER_ID" > /dev/null
        exit 1
    fi
done

# Show built images
print_step "Built images:"
docker images | grep "oxifed-" | head -10

print_success "All Docker images built and tested successfully!"

echo ""
echo "Available images:"
for service in "${SERVICES[@]}"; do
    echo "  - oxifed-$service:local"
done

echo ""
echo "Next steps:"
echo "  - Run 'docker run -p 8080:8080 oxifed-domainservd:local' to test domainservd"
echo "  - Run 'docker run oxifed-publisherd:local' to test publisherd"
echo "  - Run 'scripts/dev-setup.sh' to start full development environment"
#!/bin/bash

# Development Environment Setup Script
# Sets up and starts the complete development environment

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

print_step "Setting up Oxifed development environment..."

# Check dependencies
print_step "Checking dependencies..."

# Check Docker
if ! command -v docker &> /dev/null; then
    print_error "Docker not found. Please install Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

if ! docker info &> /dev/null; then
    print_error "Docker daemon is not running. Please start Docker."
    exit 1
fi

# Check Docker Compose
if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
    print_error "Docker Compose not found. Please install Docker Compose."
    exit 1
fi

# Check Rust
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust: https://rustup.rs/"
    exit 1
fi

print_success "All dependencies are available"

# Stop any existing containers
print_step "Stopping existing containers..."
if docker-compose ps -q | grep -q .; then
    docker-compose down
    print_success "Stopped existing containers"
else
    print_success "No existing containers to stop"
fi

# Clean up old images (optional)
if [ "$1" = "--clean" ]; then
    print_step "Cleaning up old Docker images..."
    docker system prune -f
    print_success "Docker cleanup completed"
fi

# Build Rust services first
print_step "Building Rust services..."
cargo build --all-features --workspace
print_success "Rust services built successfully"

# Start infrastructure services first
print_step "Starting infrastructure services..."
docker-compose up -d mongodb lavinmq

# Wait for services to be ready
print_step "Waiting for infrastructure services to start..."
sleep 10

# Check MongoDB connection
print_step "Checking MongoDB connection..."
MONGODB_READY=false
for i in {1..30}; do
    if docker-compose exec -T mongodb mongosh --eval "db.adminCommand('ping')" &> /dev/null; then
        MONGODB_READY=true
        break
    fi
    sleep 2
done

if [ "$MONGODB_READY" = true ]; then
    print_success "MongoDB is ready"
else
    print_error "MongoDB failed to start within timeout"
    exit 1
fi

# Check RabbitMQ connection
print_step "Checking LavinMQ connection..."
RABBITMQ_READY=false
for i in {1..30}; do
    if curl -f http://localhost:15672 &> /dev/null; then
        RABBITMQ_READY=true
        break
    fi
    sleep 2
done

if [ "$RABBITMQ_READY" = true ]; then
    print_success "LavinMQ is ready"
else
    print_warning "LavinMQ management interface not accessible, but may still be working"
fi

# Start application services
print_step "Starting application services..."
docker-compose up -d domainservd publisherd

# Wait for application services
print_step "Waiting for application services to start..."
sleep 5

# Check domainservd health
print_step "Checking domainservd health..."
DOMAINSERVD_READY=false
for i in {1..30}; do
    if curl -f http://localhost:8080/health &> /dev/null 2>&1 || curl -f http://localhost:8080 &> /dev/null 2>&1; then
        DOMAINSERVD_READY=true
        break
    fi
    sleep 2
done

if [ "$DOMAINSERVD_READY" = true ]; then
    print_success "domainservd is ready"
else
    print_warning "domainservd health check failed, but service may still be starting"
fi

# Show running containers
print_step "Development environment status:"
docker-compose ps

print_success "Development environment is ready!"

echo ""
echo "Services available:"
echo "  📊 MongoDB:           mongodb://localhost:27017"
echo "  🐰 LavinMQ:           amqp://localhost:5672"
echo "  🌐 LavinMQ Management: http://localhost:15672"
echo "  🏠 Domain Service:     http://localhost:8080"
echo "  📤 Publisher Service:  Running in background"

echo ""
echo "Useful commands:"
echo "  🔍 View logs:         docker-compose logs -f [service]"
echo "  🛑 Stop environment:  docker-compose down"
echo "  🔄 Restart service:   docker-compose restart [service]"
echo "  📋 Service status:    docker-compose ps"

echo ""
echo "Development workflow:"
echo "  1. Make code changes"
echo "  2. Run 'scripts/test-local.sh' to test changes"
echo "  3. Run 'docker-compose up -d --build [service]' to rebuild specific service"
echo "  4. Test your changes against the running environment"

echo ""
echo "To stop the development environment:"
echo "  docker-compose down"
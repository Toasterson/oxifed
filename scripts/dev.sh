#!/bin/bash

# Main Development Helper Script
# Provides convenient commands for common development tasks

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
NC='\033[0m' # No Color

# Function to print colored output
print_header() {
    echo -e "${BOLD}${BLUE}$1${NC}"
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

# Show help
show_help() {
    print_header "Oxifed Development Helper"
    echo ""
    echo "Usage: $0 <command> [options]"
    echo ""
    echo "Commands:"
    echo "  setup             Set up the development environment"
    echo "  test              Run all tests locally"
    echo "  format            Format code and apply fixes"
    echo "  build             Build Docker images locally"
    echo "  start             Start development services"
    echo "  stop              Stop development services"
    echo "  restart           Restart development services"
    echo "  logs [service]    View logs (optionally for specific service)"
    echo "  status            Show status of all services"
    echo "  clean [options]   Clean up development environment"
    echo "  run <service>     Run a specific service locally"
    echo "  shell <service>   Open shell in service container"
    echo "  db                Connect to MongoDB shell"
    echo "  mq                Open RabbitMQ management interface"
    echo ""
    echo "Examples:"
    echo "  $0 setup                    # Initial setup"
    echo "  $0 test                     # Run tests"
    echo "  $0 start                    # Start all services"
    echo "  $0 logs domainservd         # View domainservd logs"
    echo "  $0 run domainservd          # Run domainservd locally"
    echo "  $0 clean --docker           # Clean only Docker resources"
    echo ""
}

# Check if we're in the project root
check_project_root() {
    if [ ! -f "Cargo.toml" ]; then
        print_error "This script must be run from the project root directory"
        exit 1
    fi
}

# Setup development environment
setup_dev() {
    print_header "Setting up development environment..."
    ./scripts/dev-setup.sh "$@"
}

# Run tests
run_tests() {
    print_header "Running tests..."
    ./scripts/test-local.sh
}

# Format code
format_code() {
    print_header "Formatting code..."
    ./scripts/format-fix.sh "$@"
}

# Build Docker images
build_images() {
    print_header "Building Docker images..."
    ./scripts/docker-build.sh
}

# Start services
start_services() {
    print_header "Starting development services..."
    docker-compose up -d
    print_success "Services started"
    echo ""
    echo "Services available:"
    echo "  🌐 Domain Service:     http://localhost:8080"
    echo "  📊 MongoDB:           mongodb://localhost:27017"
    echo "  🐰 LavinMQ Management: http://localhost:15672"
}

# Stop services
stop_services() {
    print_header "Stopping development services..."
    docker-compose down
    print_success "Services stopped"
}

# Restart services
restart_services() {
    print_header "Restarting development services..."
    docker-compose restart "$@"
    print_success "Services restarted"
}

# View logs
view_logs() {
    if [ -n "$1" ]; then
        print_header "Viewing logs for $1..."
        docker-compose logs -f "$1"
    else
        print_header "Viewing all logs..."
        docker-compose logs -f
    fi
}

# Show status
show_status() {
    print_header "Development environment status:"
    docker-compose ps
    echo ""
    
    # Check service health
    print_header "Service health checks:"
    
    # Check domainservd
    if curl -s http://localhost:8080/health >/dev/null 2>&1 || curl -s http://localhost:8080 >/dev/null 2>&1; then
        print_success "domainservd is responding"
    else
        print_warning "domainservd is not responding"
    fi
    
    # Check MongoDB
    if docker-compose exec -T mongodb mongosh --eval "db.adminCommand('ping')" >/dev/null 2>&1; then
        print_success "MongoDB is responding"
    else
        print_warning "MongoDB is not responding"
    fi
    
    # Check LavinMQ
    if curl -s http://localhost:15672 >/dev/null 2>&1; then
        print_success "LavinMQ management is responding"
    else
        print_warning "LavinMQ management is not responding"
    fi
}

# Clean up
cleanup() {
    print_header "Cleaning up development environment..."
    ./scripts/cleanup.sh "$@"
}

# Run service locally
run_service() {
    if [ -z "$1" ]; then
        print_error "Please specify a service to run"
        echo "Available services: domainservd, publisherd, oxiadm"
        exit 1
    fi
    
    print_header "Running $1 locally..."
    cargo run --bin "$1"
}

# Open shell in container
open_shell() {
    if [ -z "$1" ]; then
        print_error "Please specify a service container"
        echo "Available services: domainservd, publisherd, mongodb, lavinmq"
        exit 1
    fi
    
    print_header "Opening shell in $1 container..."
    docker-compose exec "$1" /bin/bash
}

# Connect to MongoDB
connect_db() {
    print_header "Connecting to MongoDB..."
    docker-compose exec mongodb mongosh
}

# Open RabbitMQ management
open_mq() {
    print_header "Opening LavinMQ management interface..."
    if command -v xdg-open >/dev/null 2>&1; then
        xdg-open http://localhost:15672
    elif command -v open >/dev/null 2>&1; then
        open http://localhost:15672
    else
        echo "Please open http://localhost:15672 in your browser"
    fi
}

# Main script logic
main() {
    check_project_root
    
    if [ $# -eq 0 ]; then
        show_help
        exit 0
    fi
    
    case "$1" in
        setup)
            shift
            setup_dev "$@"
            ;;
        test)
            shift
            run_tests "$@"
            ;;
        format)
            shift
            format_code "$@"
            ;;
        build)
            shift
            build_images "$@"
            ;;
        start)
            shift
            start_services "$@"
            ;;
        stop)
            shift
            stop_services "$@"
            ;;
        restart)
            shift
            restart_services "$@"
            ;;
        logs)
            shift
            view_logs "$@"
            ;;
        status)
            shift
            show_status "$@"
            ;;
        clean)
            shift
            cleanup "$@"
            ;;
        run)
            shift
            run_service "$@"
            ;;
        shell)
            shift
            open_shell "$@"
            ;;
        db)
            shift
            connect_db "$@"
            ;;
        mq)
            shift
            open_mq "$@"
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            print_error "Unknown command: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

main "$@"
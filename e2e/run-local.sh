#!/bin/bash

# E2E Test Runner for Oxifed
# This script runs the end-to-end federation tests locally using Docker Compose

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.e2e.yml"
COMPOSE_PROJECT_NAME="oxifed-e2e"
TEST_TIMEOUT=300  # 5 minutes timeout for tests

# Default values
CLEANUP=true
BUILD=true
VERBOSE=false
TEST_FILTER=""
KEEP_RUNNING=false

# Function to print colored output
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Run end-to-end federation tests for Oxifed locally using Docker Compose.

OPTIONS:
    -h, --help          Show this help message
    -n, --no-cleanup    Don't cleanup containers after tests
    -s, --skip-build    Skip building Docker images
    -v, --verbose       Enable verbose output
    -f, --filter TEST   Run only specific test (e.g., test_domain_resolution)
    -k, --keep-running  Keep containers running after tests (implies --no-cleanup)
    -l, --logs          Show container logs during execution
    -t, --timeout SEC   Set test timeout in seconds (default: 300)

EXAMPLES:
    # Run all tests with default settings
    $0

    # Run specific test and keep containers running
    $0 --filter test_domain_resolution --keep-running

    # Skip build and run with verbose output
    $0 --skip-build --verbose

    # Run tests and show logs
    $0 --logs

EOF
}

# Function to cleanup containers
cleanup() {
    if [ "$CLEANUP" = true ]; then
        print_info "Cleaning up containers..."
        docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" down -v --remove-orphans
        print_success "Cleanup complete"
    else
        print_warning "Skipping cleanup - containers are still running"
        print_info "To manually cleanup, run:"
        echo "    docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME down -v"
    fi
}

# Function to check Docker and Docker Compose
check_dependencies() {
    print_info "Checking dependencies..."

    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed or not in PATH"
        exit 1
    fi

    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose is not installed or not in PATH"
        exit 1
    fi

    # Check if Docker daemon is running
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker daemon is not running"
        exit 1
    fi

    print_success "All dependencies are available"
}

# Function to build Docker images
build_images() {
    if [ "$BUILD" = true ]; then
        print_info "Building Docker images..."
        cd "$PROJECT_ROOT"

        # Build domainservd
        print_info "Building domainservd image..."
        docker build -f docker/domainservd/Dockerfile -t oxifed-domainservd:e2e .

        # Build publisherd
        print_info "Building publisherd image..."
        docker build -f docker/publisherd/Dockerfile -t oxifed-publisherd:e2e .

        # Build test runner
        print_info "Building test runner image..."
        docker build -f e2e/Dockerfile.test -t oxifed-test-runner:e2e .

        print_success "Docker images built successfully"
    else
        print_warning "Skipping Docker image build"
    fi
}

# Function to start services
start_services() {
    print_info "Starting services with Docker Compose..."

    local compose_cmd="docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME"

    # Start infrastructure services first
    $compose_cmd up -d mongodb rabbitmq

    print_info "Waiting for infrastructure services to be healthy..."
    sleep 10

    # Start domain services
    $compose_cmd up -d domainservd-solarm domainservd-space domainservd-aopc

    print_info "Waiting for domain services to be healthy..."
    sleep 10

    # Start publisher services
    $compose_cmd up -d publisherd-solarm publisherd-space publisherd-aopc

    print_info "Waiting for all services to be ready..."
    sleep 5

    # Check service health
    local services=("mongodb" "rabbitmq" "domainservd-solarm" "domainservd-space" "domainservd-aopc")
    for service in "${services[@]}"; do
        if docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" ps | grep -q "$service.*Up"; then
            print_success "$service is running"
        else
            print_error "$service is not running"
            docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs "$service"
            exit 1
        fi
    done

    print_success "All services are up and running"
}

# Function to run tests
run_tests() {
    print_info "Running E2E tests..."

    local test_cmd="cargo test --test e2e_federation"

    # Add test filter if specified
    if [ -n "$TEST_FILTER" ]; then
        test_cmd="$test_cmd $TEST_FILTER"
    fi

    # Add verbose flag if specified
    if [ "$VERBOSE" = true ]; then
        test_cmd="$test_cmd -- --nocapture --test-threads=1"
    else
        test_cmd="$test_cmd -- --test-threads=1"
    fi

    print_info "Test command: $test_cmd"

    # Run tests in container
    docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" run \
        --rm \
        -e RUST_BACKTRACE=1 \
        -e RUST_LOG=debug \
        test-runner \
        bash -c "cd /app && $test_cmd"

    local test_result=$?

    if [ $test_result -eq 0 ]; then
        print_success "All tests passed!"
    else
        print_error "Tests failed with exit code $test_result"

        # Show logs on failure
        print_warning "Showing recent logs from services..."
        docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs --tail=50
    fi

    return $test_result
}

# Function to show container logs
show_logs() {
    print_info "Showing container logs..."
    docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs -f
}

# Function to run tests with timeout
run_with_timeout() {
    if command -v timeout &> /dev/null; then
        timeout "$TEST_TIMEOUT" "$@"
    else
        "$@"
    fi
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_usage
            exit 0
            ;;
        -n|--no-cleanup)
            CLEANUP=false
            shift
            ;;
        -s|--skip-build)
            BUILD=false
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -f|--filter)
            TEST_FILTER="$2"
            shift 2
            ;;
        -k|--keep-running)
            KEEP_RUNNING=true
            CLEANUP=false
            shift
            ;;
        -l|--logs)
            SHOW_LOGS=true
            shift
            ;;
        -t|--timeout)
            TEST_TIMEOUT="$2"
            shift 2
            ;;
        *)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
    esac
done

# Trap to ensure cleanup on exit
trap cleanup EXIT INT TERM

# Main execution
main() {
    print_info "Starting Oxifed E2E Test Suite"
    print_info "Project root: $PROJECT_ROOT"
    print_info "Compose file: $COMPOSE_FILE"

    # Check dependencies
    check_dependencies

    # Change to project root
    cd "$PROJECT_ROOT"

    # Build images if needed
    build_images

    # Start services
    start_services

    # Show logs in background if requested
    if [ "$SHOW_LOGS" = true ]; then
        docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs -f &
        LOGS_PID=$!
    fi

    # Run tests with timeout
    run_with_timeout run_tests
    TEST_RESULT=$?

    # Stop logs if running
    if [ -n "$LOGS_PID" ]; then
        kill $LOGS_PID 2>/dev/null || true
    fi

    if [ "$KEEP_RUNNING" = true ]; then
        print_info "Containers are still running. You can interact with them using:"
        echo ""
        echo "  # View logs"
        echo "  docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME logs -f"
        echo ""
        echo "  # Access MongoDB"
        echo "  docker exec -it mongodb-e2e mongosh --username root --password testpassword"
        echo ""
        echo "  # Access RabbitMQ Management"
        echo "  open http://localhost:15672  # Username: admin, Password: testpassword"
        echo ""
        echo "  # Test endpoints"
        echo "  curl http://localhost:8081/health  # social.solarm.org"
        echo "  curl http://localhost:8082/health  # solarm.space"
        echo "  curl http://localhost:8083/health  # social.aopc.cloud"
        echo ""
        echo "  # Stop and cleanup"
        echo "  docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME down -v"
    fi

    exit $TEST_RESULT
}

# Run main function
main

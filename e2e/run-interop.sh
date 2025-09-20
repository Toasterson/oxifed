#!/bin/bash

# Interoperability Test Runner for Oxifed
# Tests federation with other ActivityPub implementations (snac, Mitra)

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.interop.yml"
COMPOSE_PROJECT_NAME="oxifed-interop"
TEST_TIMEOUT=600  # 10 minutes timeout for interop tests

# Default values
CLEANUP=true
BUILD=true
VERBOSE=false
TEST_FILTER=""
KEEP_RUNNING=false
SHOW_LOGS=false

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

print_impl() {
    echo -e "${PURPLE}[IMPL]${NC} $1"
}

print_test() {
    echo -e "${CYAN}[TEST]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
Interoperability Test Runner for Oxifed

Usage: $0 [OPTIONS]

Run interoperability tests between Oxifed and other ActivityPub implementations.

OPTIONS:
    -h, --help          Show this help message
    -n, --no-cleanup    Don't cleanup containers after tests
    -s, --skip-build    Skip building Docker images
    -v, --verbose       Enable verbose output
    -f, --filter TEST   Run only specific test
    -k, --keep-running  Keep all services running after tests
    -l, --logs          Show container logs during execution
    -t, --timeout SEC   Set test timeout in seconds (default: 600)

IMPLEMENTATIONS TESTED:
    • Oxifed (3 instances)
      - social.solarm.org (port 8081)
      - solarm.space (port 8082)
      - social.aopc.cloud (port 8083)

    • snac - Simple ActivityPub server
      - snac.aopc.cloud (port 8084)

    • Mitra - Federated social media server
      - mitra.aopc.cloud (port 8085)

EXAMPLES:
    # Run all interop tests
    $0

    # Run specific test with verbose output
    $0 --filter test_oxifed_to_snac --verbose

    # Skip build and keep services running
    $0 --skip-build --keep-running

    # Show logs during test execution
    $0 --logs

AVAILABLE TESTS:
    • test_webfinger_discovery_interop
    • test_oxifed_to_snac_follow
    • test_oxifed_to_mitra_interaction
    • test_multi_implementation_note_federation
    • test_comprehensive_interop_scenario
    • test_error_handling_interop

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
        print_info "Building Docker images for interop testing..."
        cd "$PROJECT_ROOT"

        # Build Oxifed images
        print_impl "Building Oxifed domainservd..."
        docker build -f docker/domainservd/Dockerfile -t oxifed-domainservd:interop .

        print_impl "Building Oxifed publisherd..."
        docker build -f docker/publisherd/Dockerfile -t oxifed-publisherd:interop .

        # Build snac
        print_impl "Building snac instance..."
        docker build -f e2e/Dockerfile.snac -t snac:interop e2e/

        # Build Mitra
        print_impl "Building Mitra instance..."
        docker build -f e2e/Dockerfile.mitra -t mitra:interop e2e/

        # Build test runner
        print_impl "Building interop test runner..."
        docker build -f e2e/Dockerfile.test.interop -t oxifed-test-runner:interop .

        print_success "All Docker images built successfully"
    else
        print_warning "Skipping Docker image build"
    fi
}

# Function to start services
start_services() {
    print_info "Starting interoperability test environment..."

    local compose_cmd="docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME"

    # Start infrastructure services first
    print_info "Starting infrastructure services (MongoDB, PostgreSQL, RabbitMQ)..."
    $compose_cmd up -d mongodb postgres-mitra rabbitmq

    print_info "Waiting for databases to be ready..."
    sleep 15

    # Start Oxifed instances
    print_impl "Starting Oxifed instances..."
    $compose_cmd up -d domainservd-solarm domainservd-space domainservd-aopc

    sleep 10

    print_impl "Starting Oxifed publishers..."
    $compose_cmd up -d publisherd-solarm publisherd-space publisherd-aopc

    sleep 5

    # Start other ActivityPub implementations
    print_impl "Starting snac instance..."
    $compose_cmd up -d snac

    print_impl "Starting Mitra instance..."
    $compose_cmd up -d mitra

    print_info "Waiting for all services to be ready..."
    sleep 20

    # Check service health
    print_info "Verifying service health..."

    # Check Oxifed instances
    for port in 8081 8082 8083; do
        if curl -f -s http://localhost:$port/health > /dev/null 2>&1; then
            print_success "✓ Oxifed on port $port is healthy"
        else
            print_warning "✗ Oxifed on port $port is not responding"
        fi
    done

    # Check snac
    if curl -f -s "http://localhost:8084/.well-known/webfinger?resource=acct:admin@snac.aopc.cloud" > /dev/null 2>&1; then
        print_success "✓ snac is healthy"
    else
        print_warning "✗ snac is not responding (might take longer to initialize)"
    fi

    # Check Mitra
    if curl -f -s http://localhost:8085/api/v1/instance > /dev/null 2>&1; then
        print_success "✓ Mitra is healthy"
    else
        print_warning "✗ Mitra is not responding (might take longer to initialize)"
    fi

    print_success "Interoperability test environment is ready"
}

# Function to run tests
run_tests() {
    print_info "Running interoperability tests..."

    local test_cmd="cargo test --test e2e_interop"

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
        -e TEST_INTEROP=true \
        -e TEST_FILTER="$TEST_FILTER" \
        test-runner-interop

    local test_result=$?

    if [ $test_result -eq 0 ]; then
        print_success "All interoperability tests passed!"
        echo ""
        print_impl "Successfully tested federation between:"
        print_impl "  • Oxifed ↔ Oxifed"
        print_impl "  • Oxifed ↔ snac"
        print_impl "  • Oxifed ↔ Mitra"
        print_impl "  • snac ↔ Mitra"
    else
        print_error "Interoperability tests failed with exit code $test_result"

        # Show logs on failure
        if [ "$SHOW_LOGS" != true ]; then
            print_warning "Showing recent logs from services..."
            docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs --tail=50
        fi
    fi

    return $test_result
}

# Function to show implementation status
show_status() {
    print_info "Implementation Status:"
    echo ""

    echo "Oxifed Instances:"
    echo "  • social.solarm.org: http://localhost:8081"
    echo "  • solarm.space: http://localhost:8082"
    echo "  • social.aopc.cloud: http://localhost:8083"
    echo ""

    echo "Other Implementations:"
    echo "  • snac (snac.aopc.cloud): http://localhost:8084"
    echo "  • Mitra (mitra.aopc.cloud): http://localhost:8085"
    echo ""

    echo "Infrastructure:"
    echo "  • MongoDB: localhost:27017"
    echo "  • PostgreSQL (Mitra): localhost:5432"
    echo "  • RabbitMQ: localhost:5672 (Management: http://localhost:15672)"
    echo ""
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
    print_info "Starting Oxifed Interoperability Test Suite"
    print_info "Testing federation with snac and Mitra"
    print_info "Project root: $PROJECT_ROOT"
    print_info "Compose file: $COMPOSE_FILE"
    echo ""

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
    timeout "$TEST_TIMEOUT" run_tests
    TEST_RESULT=$?

    # Stop logs if running
    if [ -n "$LOGS_PID" ]; then
        kill $LOGS_PID 2>/dev/null || true
    fi

    echo ""
    if [ "$KEEP_RUNNING" = true ]; then
        show_status
        print_info "Services are still running. You can interact with them using:"
        echo ""
        echo "  # View logs"
        echo "  docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME logs -f"
        echo ""
        echo "  # Test WebFinger discovery"
        echo "  curl 'http://localhost:8084/.well-known/webfinger?resource=acct:admin@snac.aopc.cloud'"
        echo "  curl 'http://localhost:8085/.well-known/webfinger?resource=acct:admin@mitra.aopc.cloud'"
        echo ""
        echo "  # Access Mitra API"
        echo "  curl http://localhost:8085/api/v1/instance | jq"
        echo ""
        echo "  # Stop and cleanup"
        echo "  docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME down -v"
    fi

    exit $TEST_RESULT
}

# Run main function
main

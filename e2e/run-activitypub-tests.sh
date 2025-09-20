#!/bin/bash

# ActivityPub Workflow Test Runner for Oxifed
# This script runs the ActivityPub-specific E2E tests

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
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.e2e.yml"
COMPOSE_PROJECT_NAME="oxifed-e2e-activitypub"

# Test options
TEST_NAME=""
VERBOSE=false
USE_DOCKER=true
KEEP_RUNNING=false
SHOW_HELP=false

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

print_test() {
    echo -e "${PURPLE}[TEST]${NC} $1"
}

print_activity() {
    echo -e "${CYAN}[ACTIVITY]${NC} $1"
}

# Function to show usage
show_usage() {
    cat << EOF
ActivityPub Workflow Test Runner for Oxifed

Usage: $0 [OPTIONS] [TEST_NAME]

Run ActivityPub-specific E2E tests for the Oxifed federation system.

AVAILABLE TESTS:
    all                     Run all ActivityPub tests (default)
    follow-accept           Test Follow/Accept workflow
    follow-reject           Test Follow/Reject workflow
    like                    Test Like workflow
    announce                Test Announce (boost) workflow
    undo                    Test Undo workflow (Unlike, Unfollow)
    comprehensive           Run comprehensive ActivityPub workflow test

OPTIONS:
    -h, --help              Show this help message
    -v, --verbose           Enable verbose output
    -n, --native            Run tests natively (without Docker)
    -k, --keep-running      Keep services running after tests
    -d, --docker            Use Docker Compose (default)

EXAMPLES:
    # Run all ActivityPub tests
    $0

    # Run specific test with verbose output
    $0 follow-accept -v

    # Run Like workflow test and keep services running
    $0 like --keep-running

    # Run tests natively (requires services to be running)
    $0 --native comprehensive

ACTIVITYPUB WORKFLOWS TESTED:
    • Follow → Accept/Reject
    • Like/Unlike activities
    • Announce (boost/repost)
    • Undo operations
    • Reply threads
    • Complex interaction chains

EOF
}

# Function to cleanup
cleanup() {
    if [ "$KEEP_RUNNING" = false ] && [ "$USE_DOCKER" = true ]; then
        print_info "Cleaning up Docker containers..."
        docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" down -v --remove-orphans
    fi
}

# Function to start Docker services
start_docker_services() {
    print_info "Starting Docker services for ActivityPub tests..."

    cd "$PROJECT_ROOT"

    # Build images if needed
    print_info "Building Docker images..."
    docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" build

    # Start all services
    print_info "Starting services..."
    docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" up -d

    # Wait for services to be healthy
    print_info "Waiting for services to be healthy..."
    local max_retries=30
    local retry=0

    while [ $retry -lt $max_retries ]; do
        if docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" ps | grep -q "healthy"; then
            print_success "Services are healthy"
            break
        fi
        retry=$((retry + 1))
        echo -n "."
        sleep 2
    done

    if [ $retry -eq $max_retries ]; then
        print_error "Services failed to become healthy"
        docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs
        exit 1
    fi
}

# Function to run tests with Docker
run_docker_tests() {
    local test_filter=""

    case "$TEST_NAME" in
        "follow-accept")
            test_filter="test_follow_accept_workflow"
            print_test "Running Follow/Accept workflow test"
            ;;
        "follow-reject")
            test_filter="test_follow_reject_workflow"
            print_test "Running Follow/Reject workflow test"
            ;;
        "like")
            test_filter="test_like_workflow"
            print_test "Running Like workflow test"
            ;;
        "announce")
            test_filter="test_announce_workflow"
            print_test "Running Announce workflow test"
            ;;
        "undo")
            test_filter="test_undo_workflow"
            print_test "Running Undo workflow test"
            ;;
        "comprehensive")
            test_filter="test_comprehensive_activitypub_workflow"
            print_test "Running comprehensive ActivityPub workflow test"
            ;;
        "all"|"")
            test_filter=""
            print_test "Running all ActivityPub workflow tests"
            ;;
        *)
            print_error "Unknown test: $TEST_NAME"
            echo "Available tests: all, follow-accept, follow-reject, like, announce, undo, comprehensive"
            exit 1
            ;;
    esac

    # Build test command
    local test_cmd="cargo test --test e2e_federation_activitypub"

    if [ -n "$test_filter" ]; then
        test_cmd="$test_cmd $test_filter"
    fi

    if [ "$VERBOSE" = true ]; then
        test_cmd="$test_cmd -- --nocapture --test-threads=1"
    else
        test_cmd="$test_cmd -- --test-threads=1"
    fi

    print_info "Running command: $test_cmd"

    # Run tests in Docker container
    docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" run \
        --rm \
        -e RUST_BACKTRACE=1 \
        -e RUST_LOG=debug \
        test-runner \
        bash -c "cd /app && $test_cmd"

    local result=$?

    if [ $result -eq 0 ]; then
        print_success "All ActivityPub tests passed!"
        print_activity "✅ Follow/Accept workflow"
        print_activity "✅ Follow/Reject workflow"
        print_activity "✅ Like activities"
        print_activity "✅ Announce (boost) activities"
        print_activity "✅ Undo operations"
    else
        print_error "ActivityPub tests failed"
        print_warning "Showing recent logs..."
        docker-compose -f "$COMPOSE_FILE" -p "$COMPOSE_PROJECT_NAME" logs --tail=50
    fi

    return $result
}

# Function to run tests natively
run_native_tests() {
    cd "$PROJECT_ROOT"

    # Set environment variables
    export SOLARM_URL=${SOLARM_URL:-"http://localhost:8081"}
    export SPACE_URL=${SPACE_URL:-"http://localhost:8082"}
    export AOPC_URL=${AOPC_URL:-"http://localhost:8083"}
    export MONGODB_URI=${MONGODB_URI:-"mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin"}
    export AMQP_URI=${AMQP_URI:-"amqp://admin:testpassword@localhost:5672"}
    export RUST_LOG="debug"
    export RUST_BACKTRACE=1

    local test_filter=""

    case "$TEST_NAME" in
        "follow-accept")
            test_filter="test_follow_accept_workflow"
            print_test "Running Follow/Accept workflow test (native)"
            ;;
        "follow-reject")
            test_filter="test_follow_reject_workflow"
            print_test "Running Follow/Reject workflow test (native)"
            ;;
        "like")
            test_filter="test_like_workflow"
            print_test "Running Like workflow test (native)"
            ;;
        "announce")
            test_filter="test_announce_workflow"
            print_test "Running Announce workflow test (native)"
            ;;
        "undo")
            test_filter="test_undo_workflow"
            print_test "Running Undo workflow test (native)"
            ;;
        "comprehensive")
            test_filter="test_comprehensive_activitypub_workflow"
            print_test "Running comprehensive ActivityPub workflow test (native)"
            ;;
        "all"|"")
            test_filter=""
            print_test "Running all ActivityPub workflow tests (native)"
            ;;
        *)
            print_error "Unknown test: $TEST_NAME"
            echo "Available tests: all, follow-accept, follow-reject, like, announce, undo, comprehensive"
            exit 1
            ;;
    esac

    # Build test command
    local test_cmd="cargo test --test e2e_federation_activitypub"

    if [ -n "$test_filter" ]; then
        test_cmd="$test_cmd $test_filter"
    fi

    if [ "$VERBOSE" = true ]; then
        test_cmd="$test_cmd -- --nocapture --test-threads=1"
    else
        test_cmd="$test_cmd -- --test-threads=1"
    fi

    print_info "Running command: $test_cmd"

    # Run tests
    eval $test_cmd

    local result=$?

    if [ $result -eq 0 ]; then
        print_success "All ActivityPub tests passed!"
    else
        print_error "ActivityPub tests failed"
    fi

    return $result
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_usage
            exit 0
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -n|--native)
            USE_DOCKER=false
            shift
            ;;
        -k|--keep-running)
            KEEP_RUNNING=true
            shift
            ;;
        -d|--docker)
            USE_DOCKER=true
            shift
            ;;
        -*)
            print_error "Unknown option: $1"
            show_usage
            exit 1
            ;;
        *)
            TEST_NAME="$1"
            shift
            ;;
    esac
done

# Trap for cleanup
trap cleanup EXIT INT TERM

# Main execution
main() {
    print_info "🚀 ActivityPub Workflow Test Suite for Oxifed"
    print_info "Testing ActivityPub federation protocols"
    echo ""

    if [ "$USE_DOCKER" = true ]; then
        print_info "Using Docker Compose environment"
        start_docker_services
        run_docker_tests
        TEST_RESULT=$?
    else
        print_info "Running tests natively"
        print_warning "Make sure MongoDB and RabbitMQ are running"
        print_warning "Make sure domainservd and publisherd services are running"
        run_native_tests
        TEST_RESULT=$?
    fi

    echo ""
    if [ $TEST_RESULT -eq 0 ]; then
        print_success "======================================"
        print_success "  ActivityPub Test Suite: PASSED ✅  "
        print_success "======================================"

        if [ "$KEEP_RUNNING" = true ] && [ "$USE_DOCKER" = true ]; then
            echo ""
            print_info "Services are still running. You can:"
            echo "  • View logs: docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME logs -f"
            echo "  • Access MongoDB: docker exec -it mongodb-e2e mongosh"
            echo "  • Access RabbitMQ: http://localhost:15672 (admin/testpassword)"
            echo "  • Stop services: docker-compose -f $COMPOSE_FILE -p $COMPOSE_PROJECT_NAME down"
        fi
    else
        print_error "======================================"
        print_error "  ActivityPub Test Suite: FAILED ❌  "
        print_error "======================================"
    fi

    exit $TEST_RESULT
}

# Run main function
main

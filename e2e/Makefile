# E2E Test Makefile for Oxifed
# Provides convenient commands for running end-to-end federation tests

.PHONY: help test test-verbose test-docker test-filter build clean logs up down status

# Default target
help:
	@echo "Oxifed E2E Test Commands:"
	@echo ""
	@echo "  make test           - Run all E2E tests with Docker"
	@echo "  make test-verbose   - Run tests with verbose output"
	@echo "  make test-docker    - Run tests using Docker Compose only"
	@echo "  make test-filter TEST=<name> - Run specific test"
	@echo "  make test-native    - Run tests natively (requires services)"
	@echo "  make test-activitypub - Run ActivityPub workflow tests"
	@echo "  make test-all       - Run all tests including ActivityPub"
	@echo ""
	@echo "  make build          - Build Docker images"
	@echo "  make clean          - Stop and remove all containers"
	@echo "  make logs           - Show container logs"
	@echo "  make up             - Start all services"
	@echo "  make down           - Stop all services"
	@echo "  make status         - Show service status"
	@echo ""
	@echo "  make debug          - Run tests with debug output and keep running"
	@echo "  make quick          - Run tests without rebuilding images"
	@echo "  make ci             - Run tests as in CI environment"
	@echo ""

# Run all E2E tests
test:
	@echo "Running E2E tests..."
	@./run-local.sh

# Run tests with verbose output
test-verbose:
	@echo "Running E2E tests with verbose output..."
	@./run-local.sh --verbose

# Run tests using Docker Compose
test-docker:
	@echo "Starting E2E test environment with Docker Compose..."
	@docker-compose -f docker-compose.e2e.yml up --build --abort-on-container-exit --exit-code-from test-runner

# Run specific test
test-filter:
	@if [ -z "$(TEST)" ]; then \
		echo "Error: TEST variable not set. Usage: make test-filter TEST=test_name"; \
		exit 1; \
	fi
	@echo "Running test: $(TEST)"
	@./run-local.sh --filter $(TEST) --verbose

# Run tests natively
test-native:
	@echo "Running E2E tests natively..."
	@cd .. && \
	SOLARM_URL=http://localhost:8081 \
	SPACE_URL=http://localhost:8082 \
	AOPC_URL=http://localhost:8083 \
	MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin" \
	AMQP_URI="amqp://admin:testpassword@localhost:5672" \
	cargo test --test e2e_federation -- --nocapture --test-threads=1

# Run ActivityPub workflow tests
test-activitypub:
	@echo "Running ActivityPub workflow tests..."
	@cd .. && \
	SOLARM_URL=http://localhost:8081 \
	SPACE_URL=http://localhost:8082 \
	AOPC_URL=http://localhost:8083 \
	MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin" \
	AMQP_URI="amqp://admin:testpassword@localhost:5672" \
	cargo test --test e2e_federation_activitypub -- --nocapture --test-threads=1

# Run all tests including ActivityPub
test-all:
	@echo "Running all E2E tests including ActivityPub..."
	@cd .. && \
	SOLARM_URL=http://localhost:8081 \
	SPACE_URL=http://localhost:8082 \
	AOPC_URL=http://localhost:8083 \
	MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin" \
	AMQP_URI="amqp://admin:testpassword@localhost:5672" \
	cargo test --tests e2e_federation e2e_federation_activitypub -- --nocapture --test-threads=1

# Build Docker images
build:
	@echo "Building Docker images..."
	@cd .. && docker build -f docker/domainservd/Dockerfile -t oxifed-domainservd:e2e .
	@cd .. && docker build -f docker/publisherd/Dockerfile -t oxifed-publisherd:e2e .
	@cd .. && docker build -f e2e/Dockerfile.test -t oxifed-test-runner:e2e .
	@echo "Docker images built successfully"

# Clean up containers and volumes
clean:
	@echo "Cleaning up E2E test environment..."
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e down -v --remove-orphans
	@echo "Cleanup complete"

# Show logs
logs:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e logs -f

# Start services
up:
	@echo "Starting E2E test services..."
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e up -d
	@echo "Services started. Waiting for health checks..."
	@sleep 10
	@make status

# Stop services
down:
	@echo "Stopping E2E test services..."
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e down
	@echo "Services stopped"

# Show service status
status:
	@echo "Service Status:"
	@echo "---------------"
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e ps
	@echo ""
	@echo "Health Checks:"
	@echo "---------------"
	@for port in 8081 8082 8083; do \
		if curl -f -s http://localhost:$$port/health > /dev/null 2>&1; then \
			echo "✓ Service on port $$port is healthy"; \
		else \
			echo "✗ Service on port $$port is not responding"; \
		fi \
	done
	@echo ""
	@echo "MongoDB Status:"
	@if docker exec mongodb-e2e mongosh --eval "db.adminCommand('ping')" --quiet > /dev/null 2>&1; then \
		echo "✓ MongoDB is healthy"; \
	else \
		echo "✗ MongoDB is not responding"; \
	fi
	@echo ""
	@echo "RabbitMQ Status:"
	@if curl -f -s http://localhost:15672 > /dev/null 2>&1; then \
		echo "✓ RabbitMQ Management UI is accessible"; \
	else \
		echo "✗ RabbitMQ is not responding"; \
	fi

# Debug mode - run tests with debug output and keep containers running
debug:
	@echo "Running E2E tests in debug mode..."
	@./run-local.sh --verbose --keep-running --logs

# Quick test without rebuild
quick:
	@echo "Running E2E tests without rebuilding..."
	@./run-local.sh --skip-build

# Run as in CI
ci:
	@echo "Running E2E tests as in CI environment..."
	@docker-compose -f docker-compose.e2e.yml build
	@docker-compose -f docker-compose.e2e.yml up --abort-on-container-exit --exit-code-from test-runner
	@make clean

# Individual service logs
logs-solarm:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e logs -f domainservd-solarm publisherd-solarm

logs-space:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e logs -f domainservd-space publisherd-space

logs-aopc:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e logs -f domainservd-aopc publisherd-aopc

logs-infra:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e logs -f mongodb rabbitmq

# Database access
db:
	@echo "Connecting to MongoDB..."
	@docker exec -it mongodb-e2e mongosh --username root --password testpassword oxifed

# RabbitMQ management
rabbitmq:
	@echo "Opening RabbitMQ Management UI..."
	@echo "URL: http://localhost:15672"
	@echo "Username: admin"
	@echo "Password: testpassword"
	@if command -v xdg-open > /dev/null; then \
		xdg-open http://localhost:15672; \
	elif command -v open > /dev/null; then \
		open http://localhost:15672; \
	else \
		echo "Please open http://localhost:15672 in your browser"; \
	fi

# Test specific ActivityPub workflow
test-follow:
	@echo "Testing Follow/Accept workflow..."
	@cd .. && \
	SOLARM_URL=http://localhost:8081 \
	SPACE_URL=http://localhost:8082 \
	AOPC_URL=http://localhost:8083 \
	MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin" \
	AMQP_URI="amqp://admin:testpassword@localhost:5672" \
	cargo test --test e2e_federation_activitypub test_follow -- --nocapture

test-like:
	@echo "Testing Like workflow..."
	@cd .. && \
	SOLARM_URL=http://localhost:8081 \
	SPACE_URL=http://localhost:8082 \
	AOPC_URL=http://localhost:8083 \
	MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin" \
	AMQP_URI="amqp://admin:testpassword@localhost:5672" \
	cargo test --test e2e_federation_activitypub test_like -- --nocapture

test-announce:
	@echo "Testing Announce workflow..."
	@cd .. && \
	SOLARM_URL=http://localhost:8081 \
	SPACE_URL=http://localhost:8082 \
	AOPC_URL=http://localhost:8083 \
	MONGODB_URI="mongodb://root:testpassword@localhost:27017/oxifed?authSource=admin" \
	AMQP_URI="amqp://admin:testpassword@localhost:5672" \
	cargo test --test e2e_federation_activitypub test_announce -- --nocapture

# Test individual domains
test-solarm:
	@echo "Testing social.solarm.org..."
	@curl -f http://localhost:8081/health && echo "✓ Health check passed" || echo "✗ Health check failed"
	@curl -f "http://localhost:8081/.well-known/webfinger?resource=acct:test@social.solarm.org" && echo "✓ WebFinger works" || echo "✗ WebFinger failed"

test-space:
	@echo "Testing solarm.space..."
	@curl -f http://localhost:8082/health && echo "✓ Health check passed" || echo "✗ Health check failed"
	@curl -f "http://localhost:8082/.well-known/webfinger?resource=acct:test@solarm.space" && echo "✓ WebFinger works" || echo "✗ WebFinger failed"

test-aopc:
	@echo "Testing social.aopc.cloud..."
	@curl -f http://localhost:8083/health && echo "✓ Health check passed" || echo "✗ Health check failed"
	@curl -f "http://localhost:8083/.well-known/webfinger?resource=acct:test@social.aopc.cloud" && echo "✓ WebFinger works" || echo "✗ WebFinger failed"

# Restart specific service
restart-solarm:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e restart domainservd-solarm publisherd-solarm

restart-space:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e restart domainservd-space publisherd-space

restart-aopc:
	@docker-compose -f docker-compose.e2e.yml -p oxifed-e2e restart domainservd-aopc publisherd-aopc

# Pull latest images
pull:
	@echo "Pulling latest base images..."
	@docker pull mongo:8
	@docker pull cloudamqp/lavinmq:latest
	@docker pull rust:1.75-bookworm

# Full reset - clean everything and rebuild
reset:
	@echo "Performing full reset..."
	@make clean
	@docker system prune -f
	@make build
	@echo "Reset complete"

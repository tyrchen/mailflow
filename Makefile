.PHONY: build test lint fmt clean check lambda deploy-infra help

# Default target
all: check build

# Build the project
build:
	@echo "Building project..."
	@cargo build

# Build release binary
build-release:
	@echo "Building release binary..."
	@cargo build --release

# Run tests
test:
	@echo "Running tests..."
	@cargo test

# Run clippy linter
lint:
	@echo "Running clippy..."
	@cargo clippy -- -D warnings

# Check code formatting
fmt:
	@echo "Checking formatting..."
	@cargo fmt --check

# Format code
fmt-fix:
	@echo "Formatting code..."
	@cargo fmt

# Run all checks (fmt, lint, test)
check: fmt lint test
	@echo "All checks passed!"

# Build for AWS Lambda (ARM64) - both worker and API
lambda:
	@echo "Building Mailflow Lambda functions for ARM64..."
	@echo ""
	@echo "📦 Building mailflow-worker..."
	@cargo build --release --package mailflow-worker --target aarch64-unknown-linux-gnu
	@echo ""
	@echo "📦 Building mailflow-api..."
	@cargo build --release --package mailflow-api --target aarch64-unknown-linux-gnu
	@echo ""
	@echo "📦 Packaging Lambda functions..."
	@mkdir -p assets
	@cp ~/.target/aarch64-unknown-linux-gnu/release/bootstrap assets/bootstrap
	@cd assets && zip -j bootstrap.zip bootstrap && rm bootstrap
	@cp ~/.target/aarch64-unknown-linux-gnu/release/bootstrap assets/api-bootstrap
	@cd assets && zip -j api-bootstrap.zip api-bootstrap && rm api-bootstrap
	@echo ""
	@echo "✅ Lambda functions packaged:"
	@ls -lh assets/*.zip
	@echo ""
	@echo "To deploy: cd infra && pulumi up"

# Build for AWS Lambda (x86_64) - both worker and API
lambda-x86:
	@echo "Building Mailflow Lambda functions for x86_64..."
	@echo ""
	@echo "📦 Building mailflow-worker..."
	@cargo build --release --package mailflow-worker --target x86_64-unknown-linux-gnu
	@echo ""
	@echo "📦 Building mailflow-api..."
	@cargo build --release --package mailflow-api --target x86_64-unknown-linux-gnu
	@echo ""
	@echo "📦 Packaging Lambda functions..."
	@mkdir -p assets
	@cp ~/.target/x86_64-unknown-linux-gnu/release/bootstrap assets/bootstrap
	@cd assets && zip -j bootstrap.zip bootstrap && rm bootstrap
	@cp ~/.target/x86_64-unknown-linux-gnu/release/bootstrap assets/api-bootstrap
	@cd assets && zip -j api-bootstrap.zip api-bootstrap && rm api-bootstrap
	@echo ""
	@echo "✅ Lambda functions packaged:"
	@ls -lh assets/*.zip
	@echo ""
	@echo "To deploy: cd infra && pulumi up"

# Deploy infrastructure with Pulumi
deploy-infra:
	@echo "Deploying infrastructure..."
	@cd infra && pulumi up

# Clean build artifacts
clean:
	@echo "Cleaning..."
	@cargo clean
	@rm -rf assets/*.zip

# Run cargo audit for security vulnerabilities
audit:
	@echo "Auditing dependencies..."
	@cargo audit

# Update dependencies
update:
	@echo "Updating dependencies..."
	@cargo update

# Update submodules
update-submodule:
	@git submodule update --init --recursive --remote

# Release (original workflow)
release:
	@cargo release tag --execute
	@git cliff -o CHANGELOG.md
	@git commit -a -n -m "Update CHANGELOG.md" || true
	@git push origin master
	@cargo release push --execute

# E2E Tests
e2e-setup:
	@echo "Setting up E2E test environment..."
	@cd e2e && uv sync
	@echo "✅ E2E setup complete"
	@echo "Next: Configure e2e/.env with AWS credentials"

e2e-test:
	@echo "Running all E2E tests..."
	@echo "Note: Set RUN_E2E_TESTS=1 to enable AWS tests"
	@cd e2e && RUN_E2E_TESTS=1 uv run pytest tests/e2e/ -v -s

e2e-test-dry:
	@echo "Running E2E tests in dry-run mode (skip AWS calls)..."
	@cd e2e && uv run pytest tests/e2e/ -v

e2e-smoke:
	@echo "Running smoke test (quick verification)..."
	@cd e2e && RUN_E2E_TESTS=1 uv run pytest tests/e2e/test_e2e_inbound.py::test_complete_inbound_flow -v -s

e2e-security:
	@echo "Running security E2E tests..."
	@cd e2e && RUN_E2E_TESTS=1 uv run pytest tests/e2e/ -v -m security

e2e-slow:
	@echo "Running slow/load E2E tests..."
	@cd e2e && RUN_E2E_TESTS=1 uv run pytest tests/e2e/ -v -m slow

e2e-list:
	@echo "Listing all E2E tests..."
	@cd e2e && uv run pytest tests/e2e/ --collect-only -q

e2e-clean:
	@echo "Cleaning E2E test artifacts..."
	@cd e2e && find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	@cd e2e && find . -type f -name "*.pyc" -delete 2>/dev/null || true
	@cd e2e && rm -rf .pytest_cache .coverage htmlcov
	@echo "✅ E2E cleanup complete"

# Help
help:
	@echo "Available targets:"
	@echo ""
	@echo "Rust Build & Test:"
	@echo "  build          - Build the project"
	@echo "  build-release  - Build release binary"
	@echo "  test           - Run Rust unit tests"
	@echo "  lint           - Run clippy linter"
	@echo "  fmt            - Check code formatting"
	@echo "  fmt-fix        - Format code"
	@echo "  check          - Run all checks (fmt, lint, test)"
	@echo ""
	@echo "Deployment:"
	@echo "  lambda         - Build Lambda functions (worker + API) for ARM64"
	@echo "  lambda-x86     - Build Lambda functions for x86_64"
	@echo "  deploy-infra   - Deploy infrastructure with Pulumi"
	@echo ""
	@echo "E2E Testing:"
	@echo "  e2e-setup      - Setup E2E test environment"
	@echo "  e2e-test       - Run all E2E and integration tests"
	@echo "  e2e-test-only  - Run E2E tests only"
	@echo "  e2e-integration - Run integration tests only"
	@echo "  e2e-security   - Run security tests"
	@echo "  e2e-smoke      - Quick smoke test"
	@echo "  e2e-list       - List all E2E tests"
	@echo "  e2e-clean      - Clean E2E artifacts"
	@echo ""
	@echo "Maintenance:"
	@echo "  clean          - Clean build artifacts"
	@echo "  audit          - Audit dependencies for vulnerabilities"
	@echo "  update         - Update dependencies"
	@echo "  help           - Show this help message"

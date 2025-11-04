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
	@mkdir -p infra/assets
	@echo "📦 Building mailflow-worker..."
	@CARGO_TARGET_DIR=./target cargo lambda build --release --arm64 --package mailflow-worker
	@cp target/lambda/bootstrap/bootstrap infra/assets/bootstrap
	@cd infra/assets && zip -j mailflow-worker.zip bootstrap && rm bootstrap
	@echo ""
	@echo "📦 Building mailflow-api..."
	@CARGO_TARGET_DIR=./target cargo lambda build --release --arm64 --package mailflow-api
	@cp target/lambda/bootstrap/bootstrap infra/assets/bootstrap
	@cd infra/assets && zip -j mailflow-api.zip bootstrap && rm bootstrap
	@echo ""
	@echo "✅ Lambda functions packaged:"
	@ls -lh infra/assets/*.zip
	@echo ""
	@echo "To deploy: cd infra && pulumi up"

# Build for AWS Lambda (x86_64) - both worker and API
lambda-x86:
	@echo "Building Mailflow Lambda functions for x86_64..."
	@echo ""
	@mkdir -p infra/assets
	@echo "📦 Building mailflow-worker..."
	@CARGO_TARGET_DIR=./target cargo lambda build --release --x86-64 --package mailflow-worker
	@cp target/lambda/bootstrap/bootstrap infra/assets/bootstrap
	@cd infra/assets && zip -j mailflow-worker.zip bootstrap && rm bootstrap
	@echo ""
	@echo "📦 Building mailflow-api..."
	@CARGO_TARGET_DIR=./target cargo lambda build --release --x86-64 --package mailflow-api
	@cp target/lambda/bootstrap/bootstrap infra/assets/bootstrap
	@cd infra/assets && zip -j mailflow-api.zip bootstrap && rm bootstrap
	@echo ""
	@echo "✅ Lambda functions packaged:"
	@ls -lh infra/assets/*.zip
	@echo ""
	@echo "To deploy: cd infra && pulumi up"

# Build dashboard frontend
dashboard-build:
	@echo "Building dashboard frontend..."
	@cd dashboard && npm install && npm run build
	@echo "✅ Dashboard built to dashboard/dist/"

# Deploy dashboard to S3
dashboard-deploy: dashboard-build
	@echo "Deploying dashboard to S3..."
	@aws s3 sync dashboard/dist/ s3://mailflow-dashboard-$(ENVIRONMENT)/ --delete
	@echo "✅ Dashboard deployed to S3"
	@echo "Note: CloudFront cache may need to be invalidated"

# Deploy infrastructure with Pulumi
deploy-infra: lambda
	@echo "Deploying infrastructure..."
	@cd infra && pulumi up

# Full deployment (Lambda + Dashboard + Infrastructure)
deploy: lambda dashboard-build deploy-infra dashboard-deploy
	@echo "✅ Full deployment complete!"

# Clean build artifacts
clean:
	@echo "Cleaning..."
	@cargo clean
	@rm -rf infra/assets/*.zip

# Run cargo audit for security vulnerabilities
audit:
	@echo "Auditing Rust dependencies..."
	@cargo audit

# Run dashboard security audit
audit-dashboard:
	@echo "Auditing dashboard dependencies..."
	@cd dashboard && yarn audit

# Run all security audits
audit-all: audit audit-dashboard
	@echo "✅ All security audits complete"

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
	@echo "  lambda           - Build Lambda functions (worker + API) for ARM64"
	@echo "  lambda-x86       - Build Lambda functions for x86_64"
	@echo "  dashboard-build  - Build dashboard frontend (React)"
	@echo "  dashboard-deploy - Deploy dashboard to S3"
	@echo "  deploy-infra     - Deploy infrastructure with Pulumi"
	@echo "  deploy           - Full deployment (Lambda + Dashboard + Infra)"
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
	@echo "Security & Maintenance:"
	@echo "  audit            - Audit Rust dependencies"
	@echo "  audit-dashboard  - Audit dashboard dependencies"
	@echo "  audit-all        - Run all security audits"
	@echo "  clean            - Clean build artifacts"
	@echo "  update           - Update dependencies"
	@echo "  help             - Show this help message"

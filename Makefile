COMPOSE ?= docker compose

.PHONY: up down ps logs fmt test lint migrate backup restore version version-patch version-minor version-major version-set registry-up registry-sync

up:
	$(COMPOSE) up -d

registry-up:
	$(COMPOSE) -f docker-compose.yml -f docker-compose.local-registry.yml up -d registry

registry-sync:
	./scripts/registry_sync.sh

down:
	$(COMPOSE) down

ps:
	$(COMPOSE) ps

logs:
	$(COMPOSE) logs -f

migrate:
	$(COMPOSE) exec -T postgres sh -c 'psql -U "$$POSTGRES_USER" -d "$$POSTGRES_DB" -f /migrations/0001_init.sql -f /migrations/0002_multi_project_schemas.sql'

backup:
	./scripts/backup.sh

restore:
	./scripts/restore.sh $(BACKUP)

fmt:
	cargo fmt --all --manifest-path services/common/Cargo.toml
	cargo fmt --all --manifest-path services/identity-service/Cargo.toml
	cargo fmt --all --manifest-path services/scheduler-service/Cargo.toml
	cargo fmt --all --manifest-path services/telemetry-service/Cargo.toml
	cargo fmt --all --manifest-path services/validator-service/Cargo.toml
	cargo fmt --all --manifest-path client/agent/Cargo.toml

test:
	cargo test --manifest-path services/common/Cargo.toml
	cargo test --manifest-path services/identity-service/Cargo.toml
	cargo test --manifest-path services/scheduler-service/Cargo.toml
	cargo test --manifest-path services/telemetry-service/Cargo.toml
	cargo test --manifest-path services/validator-service/Cargo.toml
	cargo test --manifest-path client/agent/Cargo.toml

lint:
	cargo clippy --manifest-path services/common/Cargo.toml --all-targets -- -D warnings
	cargo clippy --manifest-path services/identity-service/Cargo.toml --all-targets -- -D warnings
	cargo clippy --manifest-path services/scheduler-service/Cargo.toml --all-targets -- -D warnings
	cargo clippy --manifest-path services/telemetry-service/Cargo.toml --all-targets -- -D warnings
	cargo clippy --manifest-path services/validator-service/Cargo.toml --all-targets -- -D warnings
	cargo clippy --manifest-path client/agent/Cargo.toml --all-targets -- -D warnings

version:
	./scripts/version.sh current

version-patch:
	./scripts/version.sh patch

version-minor:
	./scripts/version.sh minor

version-major:
	./scripts/version.sh major

version-set:
	./scripts/version.sh set $(VERSION)

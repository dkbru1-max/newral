COMPOSE ?= docker compose

.PHONY: up down ps logs fmt test lint migrate backup restore

up:
	$(COMPOSE) up -d

down:
	$(COMPOSE) down

ps:
	$(COMPOSE) ps

logs:
	$(COMPOSE) logs -f

migrate:
	$(COMPOSE) exec -T postgres sh -c 'psql -U "$$POSTGRES_USER" -d "$$POSTGRES_DB" -f /migrations/0001_init.sql'

backup:
	./scripts/backup.sh

restore:
	./scripts/restore.sh $(BACKUP)

fmt:
	@echo "fmt: placeholder"

test:
	@echo "test: placeholder"

lint:
	@echo "lint: placeholder"

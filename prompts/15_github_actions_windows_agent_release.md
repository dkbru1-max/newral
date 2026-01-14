Проверь структуру репозитория. Найди crate агента (обычно client/agent). Если crate отсутствует или назван иначе — зафиксируй правильный путь и имя бинарника.

Сделай:
1) Добавь workflow .github/workflows/agent-windows.yml:
   - triggers:
     - push tags: agent-v*
     - workflow_dispatch
   - job на windows-latest:
     - checkout
     - install Rust toolchain stable
     - cargo build --release (по правильному пути/crate)
     - упаковать бинарник в zip (newral-agent-windows.zip)
     - upload artifact
   - если это tag agent-v*: автоматически создать GitHub Release и прикрепить zip как asset.

2) README:
   - Добавь раздел “Как собрать Windows Agent через GitHub Actions”:
     - вариант A: через тег (релиз)
     - вариант B: вручную Run workflow (тестовая сборка)

3) Проверь весь MVP, убедись, что запущены все сервисы (если нет, то запусти всё что нужно для показа инвесторам) и всё работает как швейцарские часы.
4) Сделай коммит ветке main и пуш на git и github всех изменений, passphrase для SHH DKSSH
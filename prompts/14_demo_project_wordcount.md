Используй docs/PROJECT_BRIEF.md. Цель: сделать демонстрационный проект, который можно запускать и видеть в портале.

Сделай demo-проект "wordcount":
1) На сервере (scheduler-service):
   - endpoint POST /v1/demo/wordcount/start (параметр parts=5 по умолчанию)
   - endpoint GET /v1/demo/wordcount/status (прогресс)
   - endpoint GET /v1/demo/wordcount/result (итог + топ-10 слов)
   - start: генерирует большой текст (или читает из встроенного файла), делит на 5 частей, создает 5 задач в очереди проекта demo_wordcount.
   - задача содержит payload с кусочком текста и типом python_script (код счетчика слов + входные данные).

2) На агенте:
   - при получении задачи python_script выполнить sandbox runner и вернуть JSON результата (слово->count).

3) На сервере:
   - при получении результатов агрегировать (sum counters).
   - хранить прогресс/результат в БД в схеме demo проекта.

4) На портале:
   - вкладка "Investor Demo" (или "Для инвесторов"):
     - кнопка Start Demo
     - прогресс бар
     - таблица топ-10 слов по завершению

5) Smoke-test:
   - scripts/smoke_demo.sh: запускает demo и ждет результат (таймаут).

Используй docs/PROJECT_BRIEF.md как источник истины. Пиши аккуратно, проверяй сборку и запуск. Любые изменения делай через docker-compose и конфиги, без ручных правок вне репозитория.

Цель: добавить в MVP "сердце системы" — web gateway + портал Newral, доступный по IP из локальной сети (не localhost). Пока только HTTP и без авторизации, но заложить задел на HTTPS и auth.

Сделай так:
1) Добавь сервис gateway в docker-compose.yml:
   - Вариант для MVP: Caddy или Nginx как reverse proxy.
   - Gateway публикует порт 80 на хост (0.0.0.0:80->80).
   - Внутри docker сети gateway проксирует:
     /api/identity -> identity-service
     /api/scheduler -> scheduler-service
     /api/validator -> validator-service
     /api/telemetry -> telemetry-service
     / -> frontend (статический портал)
   - В будущем должны быть места для HTTPS (443), сертификатов и auth, но сейчас не включай.

2) Добавь сервис frontend (портал) в docker-compose.yml:
   - React + Vite (быстрый современный стек), сборка в Docker (multi-stage).
   - Выдача статики через nginx (внутри frontend контейнера) или через gateway.
   - Портал доступен по http://<VM_IP>/ из локальной сети.

3) Дизайн:
   - Использовать логотип newral_big_logo.png (PNG с прозрачным фоном). Лежит сейчас в /home/user/work/newral/newral_big_logo.png, но ты его перемести куда нужно.
   - Логотип в шапке, автоматически ужимается под web-стандарты (responsive).
   - Сгенерировать favicon и маленькие иконки (положить в frontend public).
   - Стиль: современный, лаконичный, “инвесторский” (много воздуха, карточки, графики-заглушки).

4) Страницы портала (пока данные можно брать из API или делать заглушки):
   - Dashboard (краткая сводка + “для инвесторов”: total agents, tasks/day, uptime, AI mode)
   - Agents (подключённые агенты)
   - Tasks (очередь задач)
   - Monitoring (статус сервисов + метрики-заглушки)
   - AI Mode (AI_OFF / AI_ADVISORY / AI_ASSISTED / AI_FULL)
   - Projects (список проектов — если уже есть API, подключи; если нет, пока заглушка)

5) CORS:
   - Предпочтительно не “звездой” на backend’ах, а через gateway: добавь заголовки CORS для /api/* в gateway.
   - В итоге браузер должен без проблем дергать /api/* с той же origin.

6) Документация:
   - README: как открыть портал с другого компьютера по IP.
   - В README укажи: нужно открыть порты 80 (позже 443).
   - Добавь docs/PORTAL.md с планом развития (auth, https, RBAC).

Проверь:
- docker compose up -d --build
- доступ с хоста: curl http://localhost/
- доступ из сети: инструкция в README (проверка через IP).

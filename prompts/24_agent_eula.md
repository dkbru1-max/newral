# Пользовательское соглашение (EULA) на первом запуске

## Необходимость EULA
Для соответствия юридическим нормам и информирования пользователей, в GUI-агент необходимо добавить окно с соглашением с конечным пользователем (End User License Agreement, EULA). В этом соглашении должно быть ясно указано, какую информацию собирает и передаёт программа (например, данные об оборудовании), и пользователь должен явно принять эти условия перед началом работы приложения.

## Реализация функции соглашения
- **Отображение при первом запуске:** При первом запуске клиентского приложения отображается модальное окно или отдельная страница с текстом EULA. Интерфейс должен предусматривать прокрутку текста (если он большой) и недоступность основных функций до принятия соглашения.
- **Кнопка принятия ("Accept"):** В окне EULA предоставляется кнопка "Accept" (или "Я принимаю условия"). Желательно сделать её активной только после того, как пользователь прокрутил текст до конца, чтобы стимулировать ознакомление. Пока пользователь не нажал "Accept", приложение должно оставаться в пассивном режиме.
- **Блокирование без согласия:** Если пользователь отказывается (например, закрывает окно или нажимает "Decline"), приложение должно завершить работу либо оставаться на экране соглашения. По сути, без принятия EULA клиент не должен подключаться к серверу и не выполнять никаких действий (в том числе не отправлять аппаратные метрики).
- **Сохранение факта согласия:** После нажатия "Accept" приложение сохраняет информацию о принятии соглашения (например, запись в файл настроек или в реестр Windows). При последующих запусках агент проверяет этот флаг и, если согласие уже было получено ранее, окно EULA больше не показывается.
- **Возможность повторного просмотра:** Желательно предоставить способ вновь открыть текст соглашения (например, пункт меню "Пользовательское соглашение"), чтобы пользователь мог пересмотреть условия в любой момент.

## Содержание соглашения
Текст EULA составляется на английском языке, так как платформа рассчитана на широкую аудиторию. В соглашении следует отразить:
- **Сбор данных:** Программа собирает техническую информацию о компьютере пользователя (аппаратные характеристики, IP-адрес и т.п.) и периодически отправляет её на сервер.
- **Цель сбора:** Указать, что эти данные нужны для работы платформы (распределения задач, мониторинга, начисления вознаграждений, обеспечения безопасности вычислений).
- **Конфиденциальность:** Заверить, что собранные данные не используются вне платформы Newral и не передаются третьим лицам, используются только в обобщённом виде для анализа работы системы.
- **Обновления:** Сообщить, что программное обеспечение может автоматически обновляться, и условия соглашения могут измениться с обновлением; продолжение использования означает согласие с обновлёнными условиями.
- **Ответственность:** Дисклеймер об отсутствии гарантий и ограничении ответственности разработчиков за возможные проблемы при использовании программы.
- **Прекращение использования:** Пользователь имеет право в любой момент прекратить использование программы, при несогласии с условиями он должен удалить/остановить использование агентского приложения.

Ниже приведён пример EULA-текста на английском языке, охватывающий перечисленные пункты:

> **End User License Agreement (EULA) for Newral Agent**  
> **Data Collection:** By using the Newral Agent software ("the Software"), you acknowledge and consent that the Software will collect and periodically transmit technical information about your computer system to the Newral central server. This includes, but is not limited to, your IP address, CPU and GPU specifications, amount of RAM, disk capacity and usage, and other hardware metrics necessary for the operation of the distributed computing platform. No personal files or sensitive personal data will be accessed or transmitted.  
>  
> **Purpose of Data:** The collected information is used to monitor node performance, ensure the integrity and efficiency of distributed computations, and improve the overall platform. It may also be used to calculate rewards or reputation scores for your contributions.  
>  
> **Privacy and Use of Data:** All collected data is used solely within the Newral platform. It will not be shared with unauthorized third parties. The data is handled in accordance with applicable privacy laws and is used only for operational analytics, security verification (e.g., detecting malicious activity), and platform research and improvements.  
>  
> **Software Updates:** This Software may automatically download and install updates to improve its functionality and security. By using the Software, you agree that such updates (and any changes they bring to data practices or functionality) are part of the Software's operation. The EULA terms may be updated with such releases, and continued use of the Software constitutes acceptance of the updated terms.  
>  
> **User Obligations:** You agree to use the Software only for its intended purpose as part of the Newral distributed computing platform. You will not attempt to reverse engineer, tamper with, or misuse the Software or the data it processes.  
>  
> **Termination:** You may stop using the Software at any time. The Newral team reserves the right to terminate your access to the platform if you violate the terms of this EULA.  
>  
> **Disclaimer of Warranty:** The Software is provided "as is" without warranty of any kind. The Newral developers disclaim all warranties, express or implied, including but not limited to the warranties of merchantability and fitness for a particular purpose. You assume all risks associated with using the Software.  
>  
> **Limitation of Liability:** In no event shall the Newral platform or its developers be liable for any damages or losses arising from the use of or inability to use the Software, even if advised of the possibility of such damages.  
>  
> **Acceptance:** By clicking "Accept" and using the Software, you indicate that you have read, understood, and agree to all the terms and conditions of this EULA. If you do not agree, do not use the Software.

*(Текст EULA приведён в качестве примера и может быть скорректирован юридически при необходимости.)*

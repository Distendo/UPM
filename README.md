# UPM — Universal Package Manager

**UPM** — это кроссплатформенный пакетный менеджер, написанный на Rust. Он позволяет устанавливать пакеты напрямую из GitHub, управлять зависимостями и поддерживать систему в актуальном состоянии.

## Возможности

- **Кроссплатформенность**: Linux, macOS, Windows, BSD
- **Собственный формат пакетов**: `.upm` с manifest.upm
- **GitHub API**: установка пакетов напрямую из репозиториев GitHub
- **Индекс пакетов**: централизованный реестр пакетов
- **Кэширование**: умное кэширование загруженных пакетов
- **SHA256 верификация**: проверка целостности пакетов
- **Зависимости**: автоматическое разрешение и установка зависимостей
- **Роллбэк**: система отката изменений
- **Многопоточность**: параллельная загрузка пакетов
- **Красивый CLI**: цвета, прогресс-бары, информативные сообщения
- **Логирование**: полное логирование всех операций

## Установка

### Из исходного кода

```bash
git clone https://github.com/Distendo/UPM.git
cd UPM
cargo build --release
sudo cp target/release/upm /usr/local/bin/
```

### Через Cargo

```bash
cargo install upm
```

## Использование

```bash
# Показать справку
upm help

# Установить пакет
upm install ripgrep

# Удалить пакет
upm remove ripgrep

# Обновить пакет
upm update ripgrep

# Обновить все пакеты
upm update

# Поиск пакетов
upm search editor

# Список установленных пакетов
upm list

# Информация о пакете
upm info nano

# Диагностика системы
upm doctor

# Очистить кэш
upm clean
```

## Команды

| Команда | Описание |
|---------|----------|
| `install <package>` | Установка пакета |
| `remove <package>` | Удаление пакета |
| `update [package]` | Обновление пакета(ов) |
| `search <query>` | Поиск в индексе |
| `list` | Список установленных пакетов |
| `info <package>` | Информация о пакете |
| `doctor` | Диагностика системы |
| `clean` | Очистка кэша |

## Формат пакета

Пакеты UPM имеют расширение `.upm` и следующую структуру:

```
package-name.upm/
├── manifest.upm     # Манифест пакета (JSON)
├── files/           # Файлы пакета
└── scripts/         # Скрипты сборки/установки
```

### Пример manifest.upm

```json
{
  "package": "hello",
  "version": "1.0.0",
  "description": "Hello World package",
  "license": "MIT",
  "platforms": ["linux", "macos", "windows", "bsd"],
  "source": {
    "url": "https://github.com/Distendo/hello-upm",
    "source_type": "github",
    "branch": "main",
    "tag": "v1.0.0"
  },
  "dependencies": [],
  "build": [
    "cc -o hello hello.c"
  ],
  "install": [
    "install -d {{prefix}}/bin",
    "cp hello {{prefix}}/bin/"
  ]
}
```

## Архитектура

```
upm/
├── src/
│   ├── main.rs              # Точка входа
│   ├── cli.rs               # CLI интерфейс (clap)
│   ├── config.rs            # Конфигурация
│   ├── logger.rs            # Логирование
│   ├── errors.rs            # Типы ошибок
│   ├── downloader.rs        # HTTP загрузчик
│   ├── database.rs          # База установленных пакетов
│   ├── verify.rs            # SHA256 верификация
│   ├── rollback.rs          # Система отката
│   ├── doctor.rs            # Диагностика системы
│   ├── api/
│   │   ├── mod.rs
│   │   └── github.rs        # GitHub REST API
│   └── package/
│       ├── mod.rs
│       ├── manifest.rs      # Парсер манифестов
│       ├── index.rs          # Индекс пакетов
│       ├── installer.rs      # Установщик
│       └── resolver.rs       # Разрешитель зависимостей
├── packages/                # Примеры пакетов
├── index/                   # Индекс пакетов
├── config/                  # Конфигурация по умолчанию
├── installed/               # Установленные пакеты
├── cache/                   # Кэш загрузок
├── logs/                    # Лог-файлы
└── README.md
```

## Переменные окружения

- `UPM_GITHUB_TOKEN` — GitHub токен для API (рекомендуется)
- `GITHUB_TOKEN` — альтернативное имя для токена
- `UPM_VERBOSE` — подробный вывод

## Разработка

```bash
# Сборка
cargo build

# Сборка (релиз)
cargo build --release

# Запуск тестов
cargo test

# Запуск диагностики
cargo run -- doctor

# Очистка кэша
cargo run -- clean
```

## Лицензия

MIT License

---

**UPM** — проект [Distendo](https://github.com/Distendo/UPM)

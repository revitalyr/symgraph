# Инструкция по настройке и запуску

## Предварительные требования

1. Установите Flutter SDK: https://flutter.dev/docs/get-started/install/windows
2. Убедитесь, что Flutter настроен для Windows:
   ```bash
   flutter doctor
   flutter config --enable-windows-desktop
   ```

## Установка зависимостей

```bash
cd gui/symgraph_viewer
flutter pub get
```

## Создание файлов для Windows

Если файлы Windows не созданы автоматически:

```bash
flutter create --platforms=windows,web .
```

## Запуск приложения

### На Windows:
```bash
flutter run -d windows
```

### В браузере (Web):
```bash
flutter run -d chrome
```

## Решение проблем

### Если появляется ошибка "No supported devices connected":

1. Убедитесь, что Windows desktop включен:
   ```bash
   flutter config --enable-windows-desktop
   ```

2. Проверьте доступные устройства:
   ```bash
   flutter devices
   ```

3. Если Windows не отображается, установите необходимые компоненты:
   ```bash
   flutter doctor
   ```
   И следуйте инструкциям для установки недостающих компонентов.

### Если возникают проблемы с SQLite:

Приложение использует `sqlite3` и `sqlite3_flutter_libs` для работы с базой данных на Windows. Убедитесь, что эти пакеты установлены:

```bash
flutter pub get
```

## Использование

1. При запуске приложения введите путь к базе данных Symgraph (например: `mydatabase.db` или полный путь `D:\work\Projects\symgraph\mydatabase.db`)
2. Граф автоматически загрузится и отобразится
3. Используйте мышь для навигации:
   - Прокрутка колесиком - масштабирование
   - Перетаскивание - перемещение по графу
   - Клик на узел - просмотр информации о символе


# Tragus — план полного порта LibrePods на Rust + GTK 4 + libadwaita

## Context

LibrePods (GPL-3.0-or-later) — самая полная open-source реализация AirPods-фич на не-Apple платформах. Android-клиент покрывает протокол AAP целиком: handshake, ANC switching, ear detection, head tracking + жесты, hearing aid с audiogram'ом, customize transparency (8-band EQ), переименование, multipoint, customize long-press. Существующий Linux Qt-клиент покрывает 50–60% этого: нет head tracking, нет UI для customize transparency и long-press, нет переименования, hearing aid вынесен в отдельный Python-скрипт из-за того, что QtBluetooth не умеет открывать L2CAP по PSM. Параллельно ведётся Rust+Iced ветка (`linux/rust`, AGPL-3.0), но она не GTK и архитектурно проблемна (большие mutable singletons, JSON-БД из протокольного слоя, нет тестов, незакрытые опкоды).

**Tragus** — нативное GNOME-приложение, переписанное с нуля на Rust + GTK 4 + libadwaita, цель которого — паритет фич с Android-клиентом, чистая архитектура и встроенный hearing aid (без отдельного Python). Bluetooth через `bluer` решает L2CAP-PSM-боль Qt-клиента в одну строку.

### Уже зафиксированные решения (на момент плана)

| Решение | Значение |
| --- | --- |
| Имя проекта | **Tragus** (часть наружного уха; crates.io / PyPI / Flathub чисты) |
| GitHub | `github.com/makoni/tragus` (репо склонирован пустым в `/home/mak/Projects/tragus/`, два initial commit'а уже есть) |
| App-ID | `me.spaceinbox.tragus` |
| Лицензия | **GPL-3.0-or-later** (тот же текст, что в LibrePods, чтобы порт из Android-кода был чист) |
| MSRV / edition | Rust 1.95+, edition 2024 |
| UI-фреймворк | gtk4-rs + libadwaita-rs (без relm4 — стандартный GNOME-Circle путь) |
| Структура | Workspace из 3 крейтов: `tragus-protocol`, `tragus-bluetooth`, `tragus` |
| Tray-стратегия | `ksni` (StatusNotifierItem) + инструкция в README про AppIndicator extension для GNOME |
| Языки v0.1 | Английский (gettext-инфраструктура с самого начала, остальные локали — после Flathub-релиза) |
| BLE proximity scan | Включён с самого начала (M7) — auto-show при появлении и батарея кейса до открытия |
| Заимствования из Rust-ветки LibrePods | **Запрещено**: ветка под AGPL-3.0, мы под GPL-3.0. Только смотреть как reference — порт делаем из Android-кода (GPL-3). |

### Авторитетные источники для портирования

- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/bluetooth/AACPManager.kt` — codec
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/bluetooth/ATTManager.kt` — GATT/ATT
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/bluetooth/BLEManager.kt` — BLE adv
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/services/AirPodsService.kt` — главный state machine
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/utils/HeadOrientation.kt` — формулы pitch/yaw
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/utils/GestureDetector.kt` — алгоритм nod/shake
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/utils/MediaController.kt` — MPRIS-эквивалент логики
- `/home/mak/Projects/librepods/android/app/src/main/java/me/kavishdevar/librepods/data/HearingAid.kt` — 104-байтная структура GATT-payload
- `/home/mak/Projects/librepods/AAP Definitions.md` — spec
- `/home/mak/Projects/librepods/docs/control_commands.md` — все Control Command identifiers
- `/home/mak/Projects/librepods/Proximity Pairing Message.md` — формат BLE adv

---

## Архитектурные принципы

**Жёсткое разделение слоёв.**

- `tragus-protocol` — без `std::io`, без `tokio`, без `bluer`. Только `&[u8] -> Result<Event, ProtocolError>` парсеры и `&Command -> Vec<u8>` энкодеры. Ошибки через `thiserror`. Никаких `unwrap`/`panic` вне `#[cfg(test)]`. Это даёт fuzz-готовность с первого дня, и любой контрибьютор может тестировать парсер без AirPods и без BlueZ.
- `tragus-bluetooth` — actor с публичным API `Daemon::spawn() -> (DaemonHandle, EventStream)`. `DaemonHandle` — клонируемый sender команд, `EventStream` — `async_channel::Receiver<DaemonEvent>` (специально не `tokio::broadcast`: `async_channel` совместим и с tokio, и с GTK main loop). Внутри — `tokio::runtime::Builder::new_current_thread`, изолированный поток. Никаких `static mut` / `OnceCell<Mutex<…>>` для соединения — состояние в actor-task.
- `tragus` (UI) — владеет actor'ом, мостит события через `glib::spawn_future_local`, обновляет одну центральную `AirPodsState` (GObject с properties). Data-binding через `gtk::Expression` в `.ui` файлах.

**Мост tokio↔GTK.** UI-слой при запуске создаёт actor, кладёт `EventStream` в `glib::spawn_future_local`, в каждом событии конвертирует в `glib::clone!`-callback, обновляющий property на ViewModel. Команды от UI идут через `DaemonHandle::send().await` внутри `spawn_future_local`. Никакого `tokio::main` — GTK остаётся owner главного потока.

**Тестируемость протокола** — парсеры покрыты табличными тестами на статических векторах из `AAP Definitions.md`. Опционально `cargo +nightly fuzz` на `Frame::parse`.

**Архитектурные ошибки Iced-ветки, которые Tragus НЕ повторит:**

- Огромный `Mutex<AACPManagerState>` со всем подряд → у нас per-concern state в actor.
- Чтение/запись JSON-БД из `receive_packet` → у нас слой персистентности отдельно от протокола.
- Парсеры логируют и `return ()` → у нас всегда `Result<Event, ProtocolError>`.
- `tokio::process::Command("bluetoothctl connect")` → у нас только `bluer` API; если оно падает — diagnose, не workaround.

---

## Milestone roadmap

### M1 — Protocol foundation (без I/O)

**Goal.** Парсеры/энкодеры всех опкодов AACP покрыты табличными тестами.

**Что входит.** Frame-обёртка `[04 00 04 00 opcode reserved payload…]`. Опкоды 0x04 (battery), 0x06 (ear detection), 0x09 (control command, 40+ identifiers), 0x0E (audio source / TiPi), 0x0F (request notifications), 0x10/0x11 (smart routing / multipoint hijack), 0x17 (head tracking IMU 70+ bytes), 0x19 (stem press), 0x1A (rename), 0x1D (information / firmware version), 0x1F (chime volume), 0x28 (conv. awareness toggle), 0x2C (hearing aid enrol), 0x2E (connected devices / TiPi), 0x30/0x31 (proximity keys req/rsp), 0x4B (conv. awareness state), 0x4D (set feature flags для Pro 2), 0x53 (EQ data 140 bytes). Constants: `AAP_PSM = 0x1001`, `HANDSHAKE`, `REQUEST_NOTIFICATIONS`, `SET_FEATURE_FLAGS_PRO2 = D7 00 00 00 00 00 00 00`.

**Файлы (создаются в `crates/tragus-protocol/src/`):**

- `lib.rs` — re-exports + публичный API (расширить существующий)
- `frame.rs` — общая обёртка opcode + payload
- `error.rs` — `ProtocolError` enum через `thiserror`
- `battery.rs` — opcode 0x04 (компоненты Left/Right/Case + статусы)
- `ear_detection.rs` — opcode 0x06
- `control_command.rs` — opcode 0x09 + `ControlIdentifier` enum (full table из `AAP Definitions.md` + `docs/control_commands.md`)
- `notifications.rs` — opcode 0x0F
- `head_tracking.rs` — opcode 0x17 (IMU bytes [43-54], pitch/yaw extraction, fixed-point Q-format → f32)
- `proximity.rs` — opcodes 0x30/0x31 (IRK + ENC_KEY)
- `hearing_aid.rs` — opcode 0x2C (control codes для toggle)
- `feature_flags.rs` — opcode 0x4D
- `eq_data.rs` — opcode 0x53 (140 bytes, 4 EQ блока для разных режимов)
- `rename.rs` — opcode 0x1A
- `information.rs` — opcode 0x1D (null-terminated UTF-8: name, model, mfg, SN, fw1, fw2, sw, app_id, ...)
- `multipoint.rs` — opcodes 0x0E, 0x10/0x11, 0x2E (TiPi packets, smart routing builders для hijack)
- `att.rs` — ATT PDU encoder/decoder (Read/Write/Notify, минимум для нашего use-case; не полный GATT-server)
- `tests/vectors/` — `.bin` файлы с захваченными пакетами из `AAP Definitions.md`

**Зависимости.** Только `thiserror` (уже добавлен).

**Demo-критерий.** `cargo test -p tragus-protocol` зелёный, ~80+ тестов. Каждый опкод имеет минимум один parse-тест и один encode-тест.

**Тестируемость.** 100% — это и есть назначение.

**Подводные камни.**
- IMU-данные имеют byte-order quirks (little-endian fixed Q-format) — сверять с `HeadOrientation.kt` побайтно. На каждом offset'е тест.
- Apple добавляет Control Identifiers молча — оставить `ControlIdentifier::Unknown(u16)` вариант, а не `match` с `_ => Err(...)`.
- Длина EQ_DATA отличается между моделями — парсер должен принимать любой valid размер.
- `0x30` в Control Command означает HRM_STATE; `0x30` как frame opcode — PROXIMITY_KEYS_REQ. Не путать — это разные слои.

---

### M2 — L2CAP transport + handshake + connection lifecycle

**Goal.** Подключение к спаренным AirPods, handshake, поток raw-фреймов в обе стороны, авто-attach при ACL connect, reconnect.

**Что входит.** Discovery: `bluer::Adapter::device_addresses()` + фильтр по `manufacturer_data` или name pattern (`"AirPods"`). L2CAP socket к PSM `0x1001`. Handshake → `REQUEST_NOTIFICATIONS` → `SET_FEATURE_FLAGS` (для Pro 2 определяем по INFORMATION). Reconnect с экспоненциальным backoff. Auto-attach при ACL connect через `bluer::AdapterEvent::DeviceAdded`/`DeviceConnected`.

**Файлы (создаются в `crates/tragus-bluetooth/src/`):**

- `lib.rs` — публичный API (расширить существующий)
- `daemon.rs` — actor: `Daemon::spawn(adapter) -> (DaemonHandle, EventStream)`, command/event enums
- `discovery.rs` — поиск paired AirPods
- `l2cap.rs` — socket wrapper через `bluer::l2cap::Stream`, `AsyncRead`/`AsyncWrite`
- `framing.rs` — читает по AACP-длине, скармливает `tragus-protocol::Frame::parse`
- `session.rs` — handshake state machine + send-loop / recv-loop
- `error.rs` — `TransportError` (расширить существующий)

**Зависимости.** M1.

**Demo-критерий.** `cargo run -p tragus` запускает окно, в котором написано `Connected: AirPods Pro` и live battery levels (обновляются каждые ~30 сек, как присылают AirPods). Disconnect → reconnect работает без ручных действий.

**Тестируемость.** Mock `AsyncRead`/`AsyncWrite` через `tokio_test::io::Builder` — гнать handshake-байты через `session.rs` и проверять, что actor выдаёт `DaemonEvent::Connected`. L2CAP сам — integration-тест с реальным железом, опционально под feature `hardware-tests`.

**Подводные камни.**
- `bluer` требует BlueZ ≥ 5.56 для L2CAP по PSM без `CAP_NET_RAW`; на старых системах нужен `setcap cap_net_raw+eip`. В README инструкция или Flatpak с правильным манифестом.
- AirPods иногда отказывают handshake, если в системе уже открыт другой L2CAP-сокет (RFCOMM AVRCP конфликтует на старых ядрах) — таймаут handshake'а 2 секунды + понятная ошибка.

---

### M3 — Core UI: главный экран, ANC, batteries, ear detection, MPRIS

**Goal.** Главное окно показывает всё, что приходит из notifications, переключает ANC, ear detection ставит на паузу плеер.

**Что входит.** Центральный `AirPodsState` GObject с properties (`battery_left: u8`, `charging_left: bool`, `battery_right`, `charging_right`, `battery_case`, `charging_case`, `anc_mode: AncMode`, `ear_left: EarStatus`, `ear_right: EarStatus`, `connected: bool`, `device_name: String`, `model: AirPodsModel`). Главный экран: Adwaita `StatusPage` когда не подключены, `PreferencesGroup` со списком батарей + `ToggleGroup` с ANC modes (Off/On/Transparency/Adaptive) когда подключены. Ear-detection → MPRIS pause/play (логика из `MediaController.kt`: `iPausedTheMedia`/`userPlayedTheMedia` state machine).

**Файлы (создаются в `crates/tragus/src/`):**

- `application.rs` — Adwaita Application, владеет Daemon
- `state.rs` — `AirPodsState` GObject subclass через `glib::Object::derive`
- `bridge.rs` — мост `EventStream` → `state` mutations через `glib::spawn_future_local`
- `window.rs` — переписать существующий: Adwaita `NavigationView` + main page
- `ui/main_page.ui` — XML composite template
- `ui/main_page.rs` — `gtk::CompositeTemplate` Rust-side
- `ui/battery_widget.ui` + `.rs` — кастомный виджет батареи (icon + percentage + charging indicator)
- `ui/anc_selector.rs` — Adwaita `ToggleGroup` с 4 кнопками
- `mpris.rs` — MPRIS client (через `mpris2-zbus` или `zbus` напрямую — выбрать в M3 после короткой проверки crate'ов)
- `media_state.rs` — порт `MediaController.kt` state machine
- `data/resources/me.spaceinbox.tragus.gresource.xml` — gresource manifest
- `data/resources/style.css` — минимальный CSS
- `build.rs` — `glib_build_tools::compile_resources`

**Зависимости.** M1, M2.

**Demo-критерий.** Открываешь приложение — видишь батарейки, имя модели, ear status иконки. Жмёшь ANC button — AirPods переключают режим за <100ms. Вынимаешь один наушник — Spotify/любой MPRIS-плеер ставится на паузу; вставляешь — играет.

**Тестируемость.** ViewModel-тесты через `gtk::test_init()` — подаём event в bridge, проверяем property. UI-snapshot тесты (опционально). MediaController state machine — pure-функция, отдельные тесты.

**Подводные камни.**
- libadwaita `ToggleGroup` появился в 1.7 — наша feature gate `v1_8` ОК.
- `mpris2-zbus` vs `mpris-server`: `mpris-server` — для exposing своего MPRIS, нам нужен **client** (управлять чужими плеерами). Проверить и взять корректный crate.
- При вынимании одного наушника нужно знать, какой плеер сейчас активен — `org.mpris.MediaPlayer2.*` перечислять через `org.freedesktop.DBus.ListNames`, фильтр по prefix.

---

### M4 — Customize: rename, long-press, accessibility, transparency 8-band EQ, Loud Sound Reduction

**Goal.** Полный экран настроек на уровне Android-клиента.

**Что входит.** Rename (opcode 0x1A через L2CAP). Long-press actions (control commands 0x14/0x15/0x16 — single/double/triple/hold для left/right отдельно). Accessibility settings (Volume Swipe 0x25 + interval 0x23, Adaptive Volume 0x26, Software Mute 0x27, Auto-answer 0x1E, Chime Volume 0x1F, Voice Trigger 0x12, Conv. Detection 0x28). Loud Sound Reduction toggle (0x38). Customize Transparency: 8-band EQ + amplification + tone + conversation boost + ambient noise reduction (через GATT handle 0x18, 100 байт, IEEE754 LE float). ATT-канал поверх отдельного L2CAP к PSM 0x1F.

**Файлы:**

- `crates/tragus-bluetooth/src/att_session.rs` — отдельный L2CAP к PSM 0x1F, держит open пока есть UI-listener
- `crates/tragus-protocol/src/transparency.rs` — payload builder/parser для GATT 0x18 (100 байт)
- `crates/tragus/src/ui/customize_page.{ui,rs}` — родительский экран настроек, `Adwaita NavigationView` детей
- `crates/tragus/src/ui/rename_dialog.{ui,rs}` — Adwaita MessageDialog с EntryRow
- `crates/tragus/src/ui/press_hold_page.{ui,rs}` — два набора `ComboRow` (left/right × single/double/triple/hold)
- `crates/tragus/src/ui/accessibility_page.{ui,rs}` — `SwitchRow` + `ScaleRow` для каждой настройки
- `crates/tragus/src/ui/transparency_page.{ui,rs}` — 8 ползунков EQ + amp + balance + tone + conv. boost + ANR
- `crates/tragus/src/ui/loud_sound_page.{ui,rs}` — toggle + slider

**Зависимости.** M2 (L2CAP), M3 (UI shell).

**Demo-критерий.** Меняешь имя AirPods → видно в `bluetoothctl`/GNOME Settings. Двигаешь ползунки EQ → звук в Transparency меняется в реальном времени (с 100ms debounce, как в Android). Назначаешь long-press на «Next track» → жест работает в Spotify.

**Тестируемость.** ATT PDU encoder/decoder — табличные тесты. Transparency payload (100 байт) — тесты с эталонными значениями из Android `Transparency.kt`. UI — manual.

**Подводные камни.**
- GATT-handle 0x18 кэширует state на стороне AirPods, но после disconnect параметры могут «съехать» в дефолт — UI должен `Read` перед показом, а не показывать last-known.
- Iced-ветка делает write-only — у нас обязательно read+notify.
- ATT — это L2CAP с PSM 0x1F (отдельный сокет от AAP), `bluer` поддерживает.

---

### M5 — Hearing Aid + audiogram

**Goal.** Полная замена `linux/hearing-aid-adjustments.py` встроенным UI.

**Что входит.** Toggle hearing aid (control command 0x2C). Audiogram input UI (8 частот: 250, 500, 1k, 2k, 3k, 4k, 6k, 8k Hz × Left/Right ear). GATT 0x2A 104-byte payload write (`[02 02 60 00]` + 8 float L EQ + L amp + L tone + L conv. boost + L ANR + 8 float R EQ + R amp + R tone + R conv. boost + R ANR + own voice amp). Все adjustments. Импорт/экспорт audiogram в TOML/CSV (формат из Android `HearingAid.kt` как авторитет).

**Файлы:**

- `crates/tragus-protocol/src/hearing_aid.rs` — расширить: payload builder для 0x2A
- `crates/tragus/src/ui/hearing_aid_page.{ui,rs}` — главный экран
- `crates/tragus/src/ui/audiogram_widget.{ui,rs}` — кастомный draw через `gtk::DrawingArea` + snapshot (Cairo) или `gtk::GraphicsView` (если адекватно)
- `crates/tragus/src/ui/hearing_aid_adjustments.{ui,rs}` — все ползунки
- `crates/tragus/src/audiogram_io.rs` — TOML/CSV import/export, путь `~/.config/tragus/audiograms/<device-mac>.toml`

**Зависимости.** M4 (ATT-сессия).

**Demo-критерий.** Импортируешь свою аудиограмму, жмёшь Apply, слышишь разницу. Toggle on/off из главного окна. Экспорт даёт TOML, который читается обратно.

**Тестируемость.** Audiogram → 104-byte payload — табличные тесты с векторами из Android `HearingAid.kt`. CSV import — round-trip тесты.

**Подводные камни.**
- Hearing Aid требует AirPods Pro 2 firmware ≥ 6A305 — определять через INFORMATION (opcode 0x1D) и грейаут UI иначе.
- На Linux нужно `DeviceID = bluetooth:004C:0000:0000` в `/etc/bluetooth/main.conf`, иначе AirPods не активируют hearing aid feature. Inline-warning в UI с инструкцией копи-пасты.

---

### M6 — Head tracking + жесты + Conv. Awareness ducking

**Goal.** Pitch/yaw в реальном времени, nod/shake → triggers, ducking при разговоре.

**Что входит.** Включение IMU (control command для 0x17 stream). Парсинг 0x17 потока (~25 Hz). `GestureDetector` алгоритм портируется из `GestureDetector.kt`: rolling 100-sample window, peak/trough detection с dynamic threshold = max(50, min(150, variance/3)), peak height > 400, 3–4 extremes per gesture, confidence = amplitude×0.4 + rhythm×0.2 + alternation×0.2 + isolation×0.2, threshold 0.7 → "yes", otherwise "no" if horizontal motion. Conv. Awareness 0x4B → понижение громкости через `libpulse-binding` (PipeWire через pulse shim тоже работает).

**Файлы:**

- `crates/tragus-protocol/src/head_tracking.rs` — расширить: парсер IMU stream (если в M1 не закончен полностью)
- `crates/tragus/src/gesture.rs` — state machine gesture detection
- `crates/tragus/src/audio_ducker.rs` — libpulse async wrapper
- `crates/tragus/src/ui/head_tracking_page.{ui,rs}` — визуализация (2D-индикатор pitch/yaw через `gtk::DrawingArea`; 3D-голова через `gtk::GLArea` — стретч)
- `crates/tragus/src/gesture_actions.rs` — биндинг жестов на actions через `gio::SimpleAction` (вызывать D-Bus, MPRIS, exec команды)

**Зависимости.** M1, M2 (IMU stream идёт через тот же AAP).

**Demo-критерий.** Поворачиваешь голову — индикатор едет в реальном времени. Кивок при mock-уведомлении → срабатывает action. Начинаешь говорить (микрофон ловит) — Spotify тише, перестаёшь — громче.

**Тестируемость.** GestureDetector — pure-функция (последовательность IMU samples → Vec<Gesture>), отлично тестируется записанными треками (можно записать сейчас IMU поток в Android клиенте и сохранить как фикстуру).

**Подводные камни.**
- IMU stream жрёт батарею AirPods — выключать когда UI не активен и tray-only режим не подписан.
- `libpulse` async API — callback-style, обернуть в async-friendly wrapper.
- Conv. Awareness требует opcode 0x4D `SET_FEATURE_FLAGS = D7 ...` для Pro 2 чтобы работать при playing audio.

---

### M7 — System integration: tray, autostart, sleep, BLE proximity, Flatpak, CI

**Goal.** Production-ready: ставится из Flathub, фоновый сервис, автоопределение, готово к community.

**Что входит.**

1. **Tray icon** (`ksni`) с быстрым меню: ANC modes radio + battery levels read-only + "Show window" + "Quit". README раздел "GNOME users: install AppIndicator extension".
2. **XDG autostart** toggle в Preferences (creates/deletes `~/.config/autostart/me.spaceinbox.tragus.desktop`).
3. **logind sleep monitor** через `zbus` subscribe на `org.freedesktop.login1.Manager.PrepareForSleep` → graceful disconnect перед suspend, reconnect после resume.
4. **BLE proximity scan** — `bluer::Adapter::set_discovery_filter` с `Pattern("\x07\x12")` (Apple Continuity Proximity Pairing prefix), парсер manufacturer data 0x004C (формат из `Proximity Pairing Message.md`), AES-128 ECB decrypt последних 16 байт ключом ENC_KEY (полученным opcode 0x30/0x31). При появлении знакомых AirPods вне ear → notification "AirPods rядом, аккумулятор кейса 67%".
5. **Flatpak**: `flatpak/me.spaceinbox.tragus.yaml` манифест, `flatpak-cargo-generator.py` для cargo-sources.json, runtime `org.gnome.Platform//47`.
6. **AppStream metainfo** (`data/me.spaceinbox.tragus.metainfo.xml`), `.desktop`, иконки 16/32/48/128/256/scalable (нужна иконка — placeholder в M3, нормальная к M7).
7. **GitHub Actions CI** — три workflow: `ci.yml` (test + clippy + fmt + check на каждый PR), `flatpak.yml` (build flatpak с cache), `release.yml` (tag → flatpak repo + GitHub Release).

**Файлы:**

- `crates/tragus/src/tray.rs` — ksni
- `crates/tragus/src/autostart.rs` — XDG autostart helper
- `crates/tragus/src/sleep_monitor.rs` — zbus subscribe
- `crates/tragus-bluetooth/src/proximity_scanner.rs` — BLE adv parser + AES decrypt
- `crates/tragus-bluetooth/src/le.rs` — общий BLE-слой (manuf data 0x004C parsing)
- `crates/tragus-protocol/src/crypto.rs` — pure `ah()` функция (AES-128 hash для RPA), используя crate `aes`. Никакого `openssl`/FFI.
- `data/me.spaceinbox.tragus.metainfo.xml`
- `data/me.spaceinbox.tragus.desktop` (уже есть placeholder в репо — переделать)
- `data/icons/hicolor/scalable/apps/me.spaceinbox.tragus.svg` + размеры
- `data/icons/hicolor/symbolic/apps/me.spaceinbox.tragus-symbolic.svg` (для tray)
- `flatpak/me.spaceinbox.tragus.yaml`
- `flatpak/cargo-sources.json` (генерируется)
- `.github/workflows/ci.yml`
- `.github/workflows/flatpak.yml`
- `.github/workflows/release.yml`

**Зависимости.** M3+.

**Demo-критерий.** `flatpak install io.github.tragus` (из Flathub после approval) → запускается из меню → tray-иконка → подходишь к компу с AirPods в кейсе → notification «AirPods Pro поблизости, кейс 67%» → открываешь крышку → автоматическое подключение → видишь батареи. Suspend laptop → resume → reconnect автоматически.

**Тестируемость.** AES `ah()` — векторы из спецификации Bluetooth Core. AppStream metainfo — `appstreamcli validate`. .desktop — `desktop-file-validate`. Flatpak — `flatpak-builder --user --install builddir flatpak/...yaml`.

**Подводные камни.**
- Flathub требует stable runtime + reproducible build → `flatpak-cargo-generator.py` для всех cargo-deps.
- `ksni` через zbus поверх StatusNotifierItem — на GNOME без AppIndicator extension не виден. Документировать.
- BlueZ может уже резолвить RPA — проверить через `bluer::Device` properties (`address_type == Public/Random`); не дублировать crypto-работу если BlueZ уже даёт identity address.

---

## Cross-cutting concerns

**CI с M1.** Минимальный workflow: `cargo test --workspace` + `cargo clippy --workspace -- -D warnings` + `cargo fmt --check`. Flatpak job добавляется в M7. Файл `.github/workflows/ci.yml` создаётся первым же PR.

**i18n с M3.** Откладывать дороже: переписывать строки во всех виджетах позже = нудно. С M3 каждый литерал идёт через `gettext!()` (crate `gettext-rs`). `tragus.pot` обновляется в CI через `xtr`. Файлы: `po/POTFILES.in`, `po/LINGUAS` (в v0.1 пуст), `po/tragus.pot`.

**AppStream metainfo с M3.** Без него Flathub не примет; писать сразу — растёт вместе с фичами через `<release>` секции на каждый milestone.

**Иконки с M3.** Symbolic icon для tray (M7) можно дорисовать позже, но app-icon нужен раньше — без него GNOME Shell показывает `?`. К M3 — placeholder SVG (можно сгенерировать через AI на тему "tragus = stylised ear"); к M7 — окончательная.

**Persistence.** Все настройки UI и кэш audiogram'ов через `gio::Settings` + `gschema.xml` (стандартный путь GNOME-приложений). Audiogram'ы — отдельные TOML-файлы в `~/.config/tragus/audiograms/` (бинарь не лежит в gschema).

---

## Что НЕ в плане v1 (skip-list)

| Фича Android | Почему skip |
| --- | --- |
| Camera Control | Нет API на GNOME без сложной D-Bus интеграции с Cheese/Camera. Можно вернуть в v1.x как "exec arbitrary command" опцию. |
| Quick Settings tile | На GNOME эквивалент — Shell extension (вне scope). Tray покрывает use-case. |
| Home-screen widgets | GNOME Shell extensions — отдельная область. Plasma — отдельная сборка. |
| Billing/Play Premium | Не нужно — мы FOSS. |
| Xposed/RootlessSupport/RadareOffsetFinder | Android-only. Linux на BlueZ + bluer не нужен hook на runtime. |
| VendorID hook | На Linux это редактирование `/etc/bluetooth/main.conf` — описано в README, не нужен runtime hook. |
| AppListenerService | Android Accessibility API — нет аналога. |

---

## Verification (как проверить весь план)

В порядке milestone'ов:

| Milestone | Команда / действие | Ожидание |
| --- | --- | --- |
| M1 | `cargo test -p tragus-protocol` | 80+ тестов зелёные |
| M2 | `cargo run -p tragus` | Окно показывает `Connected: AirPods Pro <model>` + battery levels |
| M3 | (в окне) переключить ANC; вынуть наушник из уха | ANC переключается; Spotify/любой MPRIS-плеер ставится на паузу; вставить → играет |
| M4 | переименовать AirPods в UI; подвигать EQ-ползунки в Transparency | имя видно в `bluetoothctl info <mac>`; звук в Transparency меняется в реальном времени |
| M5 | импорт audiogram'а из CSV → Apply → toggle on | звук Hearing Aid слышно после toggle |
| M6 | повернуть голову; mock incoming-call; начать говорить с активной музыкой | индикатор движется; nod срабатывает; громкость уменьшается при речи |
| M7 | `flatpak-builder --user --install builddir flatpak/me.spaceinbox.tragus.yaml`; запуск; suspend/resume; подходить с AirPods в кейсе | устанавливается; tray-иконка; reconnect после suspend; notification при сближении |

**Критерий "v1 done":** все 7 milestones закрыты, Flatpak приложение принято на Flathub, в репо есть README с инструкциями для пользователей и контрибьюторов, AppStream metainfo проходит `appstreamcli validate`.

---

## Trade-offs (зафиксированные)

- **Один процесс с tray vs daemon+UI** → один процесс. Daemon-split дал бы CLI и multi-frontend, но удваивает IPC-боль. Архитектура actor готова к extraction позже — `Daemon` уже изолирован, обернуть его zbus-сервисом — отдельный milestone в v1.x.
- **`async-channel` vs `tokio::broadcast`** → `async-channel`: совместим с GTK main loop без `tokio::Runtime::block_on`.
- **XML composite templates vs Blueprint** → XML: Blueprint красивее, но Flatpak-runtime не всегда содержит compiler.
- **`gettext-rs` vs `fluent-rs`** → gettext: Flathub-инфра, Weblate, переводчики знают этот формат.
- **Crypto: `aes` (RustCrypto) vs `openssl`** → `aes`: pure Rust, no FFI, audited, размер бинаря меньше.
- **MPRIS client crate** → решается в M3 после короткой проверки `mpris2-zbus` vs raw `zbus`. Default — `zbus` напрямую (одна зависимость уже есть в дереве через `bluer`).

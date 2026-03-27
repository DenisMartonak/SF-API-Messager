# SF-API-Messager

An automated recruitment bot for [Shakes & Fidget](https://www.sfgame.net/) with a built-in web dashboard. Scans the Hall of Fame, filters players by configurable criteria, and sends them a guild invitation via private message -- all controllable from your browser.

Built with Rust using the [sf-api](https://github.com/) crate for game server communication and [axum](https://github.com/tokio-rs/axum) for the web UI.

## Features

- **Web Dashboard** -- configure, start, stop, and monitor the bot from `http://localhost:3000`
- **Live log output** -- color-coded, auto-scrolling log streamed over WebSocket
- **Real-time stats** -- pages scanned, players checked, matches found, messages sent
- **Live history updates** -- contacted players list updates instantly as messages are sent
- Scans the Hall of Fame page by page, up to a configurable limit
- Filters players by level range, country flag, guild membership, and active potions
- Sends a customizable private message to matching players
- Tracks contacted players in a local history file to avoid duplicate messages
- **Persistent settings** -- configuration saved to `config.json` automatically (password excluded)
- Automatic session re-login on consecutive failures
- Graceful stop -- cancel a running scan at any time from the UI
- Rate-limited requests to avoid triggering server-side protections
- **Single .exe** -- no external files needed, the web UI is embedded in the binary

## Quick Start (no Rust needed)

1. Download `SFBot.exe` from the [latest release](https://github.com/DenisMartonak/SF-API-Messager/releases/latest)
2. Run `SFBot.exe`
3. Open **http://localhost:3000** in your browser
4. Enter your SSO credentials, adjust filters, and click **Start Scan**

Your settings are saved automatically to `config.json` (password excluded) so you don't have to re-enter them next time.

## Build from Source

Requires Rust (edition 2024).

```bash
git clone https://github.com/DenisMartonak/SF-API-Messager.git
cd SF-API-Messager
cargo run
```

## Web Dashboard

The dashboard is served as a single embedded HTML page -- no external dependencies or build tools required.

| Section | Description |
|---|---|
| **Login** | SSO username and password fields |
| **Filters** | Level range, country flag checkboxes, potion/guild toggles |
| **Message** | Editable subject and content for the recruitment message |
| **Scan Settings** | Max pages to scan |
| **Controls** | Start / Stop buttons, live stats bar, clear log |
| **Log Output** | Scrollable, color-coded live log (green = success, red = error, gray = info) |
| **History** | Collapsible list of contacted players, updates in real time |

## Configuration

All settings are configured through the web UI before starting a scan:

| Setting | Default | Description |
|---|---|---|
| Min Level | `200` | Minimum player level to consider |
| Max Level | `300` | Maximum player level to consider |
| Must have potions | `true` | Only message players with active potions |
| Require no guild | `false` | Only message players without a guild |
| Country flags | `Slovakia, Czechia` | Country flags to filter by (checkboxes) |
| Message subject | `"Guild invite"` | Subject line of the private message |
| Message content | *(see default in UI)* | Body of the private message |
| Max pages | `200` | Maximum number of HoF pages to scan |

## How It Works

1. Enter your SSO credentials in the web dashboard
2. Configure filters and message content
3. Click **Start Scan**
4. The bot logs in, connects to the game server, and iterates through Hall of Fame pages
5. For each player, applies filters: already contacted, level range, guild, country flag, potions
6. Sends a private message to matching players
7. Records contacted players in `contacted.txt` to prevent future duplicates
8. All progress is streamed to the dashboard in real time via WebSocket

## Project Structure

```
.
├── src/
│   ├── main.rs              # Axum web server (REST + WebSocket endpoints)
│   ├── bot.rs               # Bot scan logic (config, filters, messaging)
│   ├── sendtest.rs          # Standalone test binary for message sending
│   └── static/
│       └── index.html       # Web dashboard (embedded at compile time)
├── sf-api/                  # Bundled sf-api crate (local dependency)
├── contacted.txt            # History of contacted player names (auto-created)
├── config.json              # Saved settings (auto-created, gitignored)
├── Cargo.toml
└── Cargo.lock
```

## License

This project is licensed for educational and personal learning purposes only. See [LICENSE](LICENSE) for details.

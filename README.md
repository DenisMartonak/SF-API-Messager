# SF-API-Messager

An automated recruitment bot for [Shakes & Fidget](https://www.sfgame.net/) that scans the Hall of Fame, filters players by configurable criteria, and sends them a guild invitation via private message.

Built with Rust using the [sf-api](https://github.com/) crate for game server communication.

## Features

- Scans the Hall of Fame page by page, up to a configurable limit
- Filters players by level range, country flag, guild membership, and active potions
- Sends a customizable private message to matching players
- Tracks contacted players in a local history file to avoid duplicate messages
- Automatic session re-login on consecutive failures
- Rate-limited requests to avoid triggering server-side protections

## Requirements

- Rust (edition 2024)
- A Shakes & Fidget SSO account

## Setup

1. Clone the repository:

```bash
git clone https://github.com/DenisMartonak/SF-API-Messager.git
cd SF-API-Messager
```

2. Create a `.env` file from the example:

```bash
cp env.example .env
```

3. Fill in your credentials in `.env`:

```
SSO_USERNAME=your_username
PASSWORD=your_password
```

4. Build and run:

```bash
cargo run
```

## Configuration

All configuration is done via constants at the top of `src/main.rs`:

| Constant | Type | Default | Description |
|---|---|---|---|
| `MIN_LEVEL` | `u32` | `200` | Minimum player level to consider |
| `MAX_LEVEL` | `u32` | `300` | Maximum player level to consider |
| `MUST_HAVE_POTIONS` | `bool` | `true` | Only message players with active potions |
| `REQUIRE_NO_GUILD` | `bool` | `false` | Only message players without a guild |
| `ACCEPTED_FLAGS` | `&[Flag]` | `Slovakia, Czechia` | Country flags to filter by |
| `MSG_SUBJECT` | `&str` | `"Guild invite"` | Subject line of the private message |
| `MSG_CONTENT` | `&str` | *(see source)* | Body of the private message |
| `MAX_PAGES_TO_SCAN` | `u32` | `200` | Maximum number of HoF pages to scan |

## How It Works

1. Logs into the Shakes & Fidget SSO with the credentials from `.env`
2. Connects to the game server using the first character on the account
3. Iterates through Hall of Fame pages
4. For each player on a page, applies the following filters in order:
   - Already contacted (checked against `contacted.txt`)
   - Level outside the configured range
   - Has a guild (if `REQUIRE_NO_GUILD` is enabled)
   - Country flag not in `ACCEPTED_FLAGS`
   - No active potions (if `MUST_HAVE_POTIONS` is enabled)
5. Sends a private message to players that pass all filters
6. Records contacted players in `contacted.txt` to prevent future duplicates

## Project Structure

```
.
├── src/
│   ├── main.rs          # Main bot logic
│   └── sendtest.rs      # Standalone test binary for message sending
├── sf-api/              # Bundled sf-api crate (local dependency)
├── contacted.txt        # History of contacted player names
├── env.example          # Template for .env credentials
├── Cargo.toml
└── Cargo.lock
```

## License

This project is provided as-is for personal use.

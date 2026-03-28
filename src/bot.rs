use sf_api::{
    command::{Command, Flag},
    gamestate::GameState,
    misc::to_sf_string,
    sso::SFAccount,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::time::sleep;

const HISTORY_FILE: &str = "contacted.txt";
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub username: String,
    pub password: String,
    pub min_level: u32,
    pub max_level: u32,
    pub must_have_potions: bool,
    pub require_no_guild: bool,
    pub accepted_flags: Vec<Flag>,
    pub msg_subject: String,
    pub msg_content: String,
    pub max_pages: u32,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BotStats {
    pub pages_scanned: u32,
    pub players_checked: u32,
    pub matches_found: u32,
    pub messages_sent: u32,
}

fn emit(tx: &broadcast::Sender<String>, level: &str, message: &str) {
    let json = serde_json::json!({
        "type": "log",
        "level": level,
        "message": message,
    });
    let _ = tx.send(json.to_string());
}

fn emit_stats(tx: &broadcast::Sender<String>, stats: &BotStats) {
    let json = serde_json::json!({
        "type": "stats",
        "data": stats,
    });
    let _ = tx.send(json.to_string());
}

fn emit_status(tx: &broadcast::Sender<String>, status: &str) {
    let json = serde_json::json!({
        "type": "status",
        "status": status,
    });
    let _ = tx.send(json.to_string());
}

fn emit_history(tx: &broadcast::Sender<String>, name: &str) {
    let json = serde_json::json!({
        "type": "history",
        "name": name,
    });
    let _ = tx.send(json.to_string());
}

fn load_history() -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(file) = File::open(HISTORY_FILE) {
        for line in io::BufReader::new(file).lines() {
            if let Ok(name) = line {
                let trimmed = name.trim().to_string();
                if !trimmed.is_empty() {
                    set.insert(trimmed);
                }
            }
        }
    }
    set
}

fn save_to_history(name: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(HISTORY_FILE) {
        let _ = writeln!(file, "{}", name);
    }
}

pub fn load_history_list() -> Vec<String> {
    let mut list = Vec::new();
    if let Ok(file) = File::open(HISTORY_FILE) {
        for line in io::BufReader::new(file).lines() {
            if let Ok(name) = line {
                let trimmed = name.trim().to_string();
                if !trimmed.is_empty() {
                    list.push(trimmed);
                }
            }
        }
    }
    list
}

fn should_stop(rx: &watch::Receiver<bool>) -> bool {
    *rx.borrow()
}

pub async fn run_scan(
    config: BotConfig,
    log_tx: broadcast::Sender<String>,
    stop_rx: watch::Receiver<bool>,
) {
    let mut stats = BotStats::default();
    emit_status(&log_tx, "running");

    let mut contacted_players = load_history();
    emit(&log_tx, "info", &format!(
        "Loaded {} players from history.", contacted_players.len()
    ));

    if config.accepted_flags.is_empty() {
        emit(&log_tx, "warn", "No flags selected — all players will be skipped!");
        emit_status(&log_tx, "idle");
        return;
    }

    emit(&log_tx, "info", "Logging in to SSO...");

    let account = match SFAccount::login(config.username.clone(), config.password.clone()).await {
        Ok(a) => a,
        Err(e) => {
            emit(&log_tx, "error", &format!("SSO login failed: {}", e));
            emit_status(&log_tx, "idle");
            return;
        }
    };

    let characters = match account.characters().await {
        Ok(c) => c,
        Err(e) => {
            emit(&log_tx, "error", &format!("Failed to fetch characters: {}", e));
            emit_status(&log_tx, "idle");
            return;
        }
    };

    let mut session = match characters.into_iter().flatten().next() {
        Some(s) => s,
        None => {
            emit(&log_tx, "error", "No character found on account!");
            emit_status(&log_tx, "idle");
            return;
        }
    };

    emit(&log_tx, "info", &format!(
        "Connecting to game server: {}...", session.server_url()
    ));

    let login_res = match session.login().await {
        Ok(r) => r,
        Err(e) => {
            emit(&log_tx, "error", &format!("Game login failed: {}", e));
            emit_status(&log_tx, "idle");
            return;
        }
    };

    let mut gs = match GameState::new(login_res) {
        Ok(g) => g,
        Err(e) => {
            emit(&log_tx, "error", &format!("Failed to init GameState: {}", e));
            emit_status(&log_tx, "idle");
            return;
        }
    };

    let own_guild = gs.guild.as_ref().map(|g| g.name.clone());
    if let Some(ref name) = own_guild {
        emit(&log_tx, "info", &format!("Your guild: {}", name));
    }

    emit(&log_tx, "success", &format!(
        "Logged in as {}. Starting HoF scan...", session.username()
    ));
    emit(&log_tx, "info", &format!(
        "Filters: Lvl {}-{}, No Guild: {}, Potions: {}",
        config.min_level, config.max_level, config.require_no_guild, config.must_have_potions
    ));
    emit(&log_tx, "info", &format!("Accepted Flags: {:?}", config.accepted_flags));
    emit_stats(&log_tx, &stats);

    let mut consecutive_failures: u32 = 0;

    for page in 0..config.max_pages {
        if should_stop(&stop_rx) {
            emit(&log_tx, "info", "Scan stopped by user.");
            break;
        }

        emit(&log_tx, "info", &format!("Scanning page {}...", page));

        let res = match session
            .send_command(Command::HallOfFamePage { page: page as usize })
            .await
        {
            Ok(r) => {
                consecutive_failures = 0;
                r
            }
            Err(e) => {
                consecutive_failures += 1;
                emit(&log_tx, "error", &format!(
                    "Failed to fetch page {} ({} consecutive failures): {}",
                    page, consecutive_failures, e
                ));

                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    emit(&log_tx, "error", "Too many failures. Attempting re-login...");
                    match session.login().await {
                        Ok(lr) => {
                            match GameState::new(lr) {
                                Ok(new_gs) => {
                                    gs = new_gs;
                                    consecutive_failures = 0;
                                    emit(&log_tx, "success", "Re-login successful.");
                                }
                                Err(e) => {
                                    emit(&log_tx, "error", &format!("Failed to re-init GameState: {}", e));
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            emit(&log_tx, "error", &format!("Re-login failed: {}. Aborting.", e));
                            break;
                        }
                    }
                }

                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if let Err(e) = gs.update(&res) {
            emit(&log_tx, "error", &format!("Failed to parse HoF data: {}", e));
            continue;
        }

        let players = gs.hall_of_fames.players.clone();

        if players.is_empty() {
            emit(&log_tx, "info", "End of Hall of Fame reached.");
            break;
        }

        stats.pages_scanned = page + 1;
        emit_stats(&log_tx, &stats);

        for player in &players {
            if should_stop(&stop_rx) {
                emit(&log_tx, "info", "Scan stopped by user.");
                emit_status(&log_tx, "idle");
                emit_stats(&log_tx, &stats);
                return;
            }

            if contacted_players.contains(&player.name) {
                continue;
            }

            if player.level < config.min_level || player.level > config.max_level {
                continue;
            }

            if config.require_no_guild && player.guild.is_some() {
                continue;
            }

            if let Some(flag) = player.flag {
                if !config.accepted_flags.contains(&flag) {
                    continue;
                }
            } else {
                continue;
            }

            stats.players_checked += 1;
            emit(&log_tx, "info", &format!(
                "Checking candidate: {} (Lvl {} | {:?})...",
                player.name, player.level, player.flag
            ));

            sleep(Duration::from_millis(500)).await;

            match session
                .send_command(Command::ViewPlayer { ident: player.name.clone() })
                .await
            {
                Ok(profile_res) => {
                    if let Err(e) = gs.update(&profile_res) {
                        emit(&log_tx, "error", &format!(
                            "Failed to parse profile for {}: {}", player.name, e
                        ));
                        continue;
                    }
                }
                Err(e) => {
                    emit(&log_tx, "error", &format!("Failed to view profile: {}", e));
                    continue;
                }
            }

            let looked_up = gs.lookup.lookup_name(&player.name);

            if let (Some(own), Some(details)) = (&own_guild, &looked_up) {
                if details.guild.as_deref() == Some(own.as_str()) {
                    emit(&log_tx, "info", &format!("Skipping {} (same guild).", player.name));
                    continue;
                }
            }

            let has_potions = looked_up
                .map(|d| d.active_potions.iter().any(|p| p.is_some()))
                .unwrap_or(false);

            if config.must_have_potions && !has_potions {
                emit(&log_tx, "info", &format!("Skipping {} (no active potions).", player.name));
                continue;
            }

            stats.matches_found += 1;
            emit(&log_tx, "success", &format!("MATCH! Sending message to {}...", player.name));
            emit_stats(&log_tx, &stats);

            let cmd = Command::Custom {
                cmd_name: "PlayerMessageSend".to_string(),
                arguments: vec![
                    player.name.clone(),
                    to_sf_string(&config.msg_subject),
                    to_sf_string(&config.msg_content),
                ],
            };

            match session.send_command(cmd).await {
                Ok(_) => {
                    stats.messages_sent += 1;
                    emit(&log_tx, "success", &format!(
                        "Message sent to {}! Waiting 40s...", player.name
                    ));
                    save_to_history(&player.name);
                    contacted_players.insert(player.name.clone());
                    emit_stats(&log_tx, &stats);
                    emit_history(&log_tx, &player.name);
                    sleep(Duration::from_secs(40)).await;
                }
                Err(e) => {
                    emit(&log_tx, "error", &format!(
                        "Failed to send message to {}: {}", player.name, e
                    ));
                    contacted_players.insert(player.name.clone());
                    emit(&log_tx, "info", "Waiting 40s...");
                    sleep(Duration::from_secs(40)).await;
                }
            }
        }

        sleep(Duration::from_secs(2)).await;
    }

    emit(&log_tx, "success", "Scan complete.");
    emit_stats(&log_tx, &stats);
    emit_status(&log_tx, "idle");
}

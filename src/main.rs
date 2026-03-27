use sf_api::{
    command::{Command, Flag},
    sso::SFAccount,
    session::Session,
    gamestate::GameState,
    misc::to_sf_string,
    error::SFError,
    response::Response,
};
use std::env;
use std::time::Duration;
use tokio::time::sleep;
use dotenv::dotenv;
use std::collections::HashSet;
use std::fs::{OpenOptions, File};
use std::io::{self, BufRead, Write};

const MIN_LEVEL: u32 = 200;
const MAX_LEVEL: u32 = 300;
const MUST_HAVE_POTIONS: bool = true;
const REQUIRE_NO_GUILD: bool = false;

const ACCEPTED_FLAGS: &[Flag] = &[Flag::Slovakia, Flag::Czechia];

const MSG_SUBJECT: &str = "Guild invite";
const MSG_CONTENT: &str = "\
SK:\n\
Ahoj,\n\
radi by sme ťa pozvali do nášho cechu. Sme aktívna komunita hráčov, ktorí sa zameriavajú na pravidelnú aktivitu, dlhodobý progres a tímovú spoluprácu. Navzájom si pomáhame, komunikujeme a spoločne sa snažíme posúvať cech aj jednotlivcov dopredu.\n\
Hľadáme hráčov, ktorí majú záujem hrať aktívne a byť súčasťou stabilného kolektívu. Ak hľadáš cech s rozumným prístupom, dobrými bonusmi a priateľskou atmosférou, budeme radi, ak sa nám ozveš.\n
EN:\n\
Hello,\n\
We would like to invite you to join our guild. We are an active community of players focused on regular activity, long-term progress, and teamwork. We support each other, communicate, and work together to move both the guild and its members forward.\n\
We would be happy to hear from you.";
const MAX_PAGES_TO_SCAN: u32 = 200;
const HISTORY_FILE: &str = "contacted.txt";
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

pub async fn send_private_message(
    session: &mut Session,
    target_name: &str,
    subject: &str,
    message: &str,
) -> Result<Response, SFError> {
    let cmd = Command::Custom {
        cmd_name: "PlayerMessageSend".to_string(),
        arguments: vec![
            target_name.to_string(),
            to_sf_string(subject),
            to_sf_string(message),
        ],
    };
    session.send_command(cmd).await
}

fn load_history() -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(file) = File::open(HISTORY_FILE) {
        let reader = io::BufReader::new(file);
        for line in reader.lines() {
            if let Ok(name) = line {
                if !name.trim().is_empty() {
                    set.insert(name.trim().to_string());
                }
            }
        }
    }
    set
}

fn save_to_history(name: &str) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(HISTORY_FILE)
    {
        if let Err(e) = writeln!(file, "{}", name) {
            eprintln!("Failed to write to history file: {}", e);
        }
    }
}

#[tokio::main]
pub async fn main() {
    dotenv().ok();

    if ACCEPTED_FLAGS.is_empty() {
        eprintln!("WARNING: ACCEPTED_FLAGS is empty — all players will be skipped!");
    }

    let mut contacted_players = load_history();
    println!("Loaded {} players from history.", contacted_players.len());

    println!("Logging in...");
    let account = SFAccount::login(
        env::var("SSO_USERNAME").expect("Missing SSO_USERNAME"),
        env::var("PASSWORD").expect("Missing PASSWORD"),
    )
    .await
    .expect("Failed to login to S&F Account");

    let characters = account.characters().await.expect("Failed to fetch characters");
    let mut session = characters
        .into_iter()
        .flatten()
        .next()
        .expect("No character found on account!");

    println!("Connecting to game server: {}...", session.server_url());
    let login_res = session.login().await.expect("Game login failed");

    let mut gs = GameState::new(login_res).expect("Failed to init GameState");

    println!(
        "Logged in as {}. Starting HOF Scan...",
        session.username()
    );
    println!(
        "Filters: Lvl {}-{}, No Guild: {}, Potions: {}",
        MIN_LEVEL, MAX_LEVEL, REQUIRE_NO_GUILD, MUST_HAVE_POTIONS
    );
    println!("Accepted Flags: {:?}", ACCEPTED_FLAGS);

    let mut consecutive_failures: u32 = 0;

    for page in 0..MAX_PAGES_TO_SCAN {
        println!("Scanning Page {}", page);

        let res = match session
            .send_command(Command::HallOfFamePage {
                page: page as usize,
            })
            .await
        {
            Ok(r) => {
                consecutive_failures = 0;
                r
            }
            Err(e) => {
                consecutive_failures += 1;
                eprintln!(
                    "Failed to fetch page {} ({} consecutive failures): {}",
                    page, consecutive_failures, e
                );

                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    eprintln!("Too many consecutive failures. Attempting re-login...");
                    match session.login().await {
                        Ok(login_res) => {
                            gs = GameState::new(login_res)
                                .expect("Failed to re-init GameState");
                            consecutive_failures = 0;
                            eprintln!("Re-login successful.");
                        }
                        Err(login_err) => {
                            eprintln!("Re-login failed: {}. Aborting scan.", login_err);
                            return;
                        }
                    }
                }

                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if let Err(e) = gs.update(&res) {
            eprintln!("Failed to parse HOF data: {}", e);
            continue;
        }

        let players = gs.hall_of_fames.players.clone();

        if players.is_empty() {
            println!("End of Hall of Fame reached.");
            break;
        }

        for player in players {
            if contacted_players.contains(&player.name) {
                continue;
            }

            if player.level < MIN_LEVEL || player.level > MAX_LEVEL {
                continue;
            }

            if REQUIRE_NO_GUILD && player.guild.is_some() {
                continue;
            }

            if let Some(player_flag) = player.flag {
                if !ACCEPTED_FLAGS.contains(&player_flag) {
                    continue;
                }
            } else {
                continue;
            }

            println!(
                "Checking candidate: {} (Lvl {} | {:?})...",
                player.name, player.level, player.flag
            );

            sleep(Duration::from_millis(500)).await;

            match session
                .send_command(Command::ViewPlayer {
                    ident: player.name.clone(),
                })
                .await
            {
                Ok(profile_res) => {
                    if let Err(e) = gs.update(&profile_res) {
                        eprintln!("Failed to parse profile for {}: {}", player.name, e);
                        continue;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to view profile: {}", e);
                    continue;
                }
            }

            let has_potions = if let Some(details) = gs.lookup.lookup_name(&player.name) {
                details.active_potions.iter().any(|p| p.is_some())
            } else {
                false
            };

            if MUST_HAVE_POTIONS && !has_potions {
                println!("Skipping (No active potions).");
                continue;
            }

            println!("MATCH! Sending message to {}...", player.name);

            match send_private_message(&mut session, &player.name, MSG_SUBJECT, MSG_CONTENT).await
            {
                Ok(_) => {
                    println!("Message sent! Waiting 40s...");
                    save_to_history(&player.name);
                    contacted_players.insert(player.name.clone());
                    sleep(Duration::from_secs(40)).await;
                }
                Err(e) => {
                    eprintln!("Failed to send message to {}: {}", player.name, e);
                    contacted_players.insert(player.name.clone());
                    println!("Waiting 40s...");
                    sleep(Duration::from_secs(40)).await;
                }
            }
        }

        sleep(Duration::from_secs(2)).await;
    }

    println!("Scan complete.");
}

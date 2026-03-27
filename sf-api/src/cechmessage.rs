use std::time::Duration;
use tokio::time::sleep;

use sf_api::{
    command::Command,
    sso::SFAccount,
};

// --- CONFIGURATION ---
const EMAIL: &str = "YOUR_EMAIL@EXAMPLE.COM";
const PASSWORD: &str = "YOUR_PASSWORD";
const MSG_CONTENT: &str = "Hello! I noticed you have potions active but no guild. Would you like to join us?";

// Filter Settings
const MIN_LEVEL: u32 = 230;
const MAX_LEVEL: u32 = 350;

#[tokio::main]
async fn main() {
    println!("Logging in via SSO...");

    // 1. Login to the S&F Account
    let account = SFAccount::login(EMAIL.to_string(), PASSWORD.to_string())
        .await
        .expect("Failed to login to SSO account");

    // 2. Fetch characters and select the first one
    // Note: account.characters() consumes the account object
    let characters = account.characters().await.expect("Failed to fetch characters");

    // We take the first character found on the account
    let mut session = match characters.into_iter().next() {
        Some(Ok(sess)) => sess,
        Some(Err(e)) => panic!("Error loading character: {}", e),
        None => panic!("No characters found on this account!"),
    };

    println!("Logged in as: {}", session.username());

    // 3. Start Scrape Loop
    let mut page = 0;

    loop {
        println!("Scanning Hall of Fame Page {}...", page);

        // Fetch the page
        if let Err(e) = session.send_command(Command::HallOfFamePage { page }).await {
            eprintln!("Failed to fetch page {}: {}", page, e);
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        // Clone the list of players so we don't hold a reference to state while sending new commands
        let players = session
            .game_state()
            .map(|gs| gs.hall_of_fames.players.clone())
            .unwrap_or_default();

        if players.is_empty() {
            println!("No more players found. Exiting.");
            break;
        }

        for player in players {
            // --- FILTER 1: Level & Guild (Available in HOF list) ---
            let level_ok = player.level >= MIN_LEVEL && player.level <= MAX_LEVEL;
            let no_guild = player.guild.is_none(); // "None" means not in a guild

            if level_ok && no_guild {
                print!("Checking candidate: {} (Lvl {})... ", player.name, player.level);

                // --- FILTER 2: Potions (Requires ViewPlayer) ---
                // We must fetch the full profile to see potions
                if let Err(_) = session.send_command(Command::ViewPlayer { ident: player.name.clone() }).await {
                    println!("Failed to inspect.");
                    continue;
                }

                // Check the Lookup table for the data we just fetched
                let has_potions = if let Some(details) = session.game_state().unwrap().lookup.lookup_name(&player.name) {
                    // active_potions is [Option<Potion>; 3]. We check if any slot is Some.
                    details.active_potions.iter().any(|p| p.is_some())
                } else {
                    false
                };

                if has_potions {
                    println!("MATCH! Has potions. Sending message.");

                    // --- ACTION: Send Message ---
                    let result = session.send_command(Command::SendMessage {
                        to: player.name.clone(),
                        msg: MSG_CONTENT.to_string(),
                    }).await;

                    match result {
                        Ok(_) => {
                            println!("Message sent to {}. Waiting 40s...", player.name);
                            sleep(Duration::from_secs(40)).await;
                        }
                        Err(e) => eprintln!("Failed to send message: {}", e),
                    }
                } else {
                    println!("Skipped (No potions).");
                }

                // Small sleep to respect the server and not look like a DDoSer
                sleep(Duration::from_millis(750)).await;
            }
        }

        // Move to next page
        page += 1;
        // Sleep between pages
        sleep(Duration::from_secs(2)).await;
    }
}
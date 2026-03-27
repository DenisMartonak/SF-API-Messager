use sf_api::{
    command::Command,
    sso::SFAccount,
    session::Session,
    gamestate::GameState,
    error::SFError,
};

use std::env;
use dotenv::dotenv;
pub async fn get_guild_from_player(
    session: &mut Session,
    player_name: &str
) -> Result<Option<String>, SFError> {
    let resp = session.send_command(Command::ViewPlayer {
        ident: player_name.to_string(),
    }).await?;
    let mut game_state = GameState::default();

    game_state.update(resp)?;

    if let Some(other_player) = game_state.lookup.lookup_name(player_name) {
        Ok(other_player.guild.clone())
    } else {
        Ok(None)
    }
}

#[tokio::main]
pub async fn main() {
    dotenv().ok();

    // --- LOGIN SEQUENCE ---
    println!("Logging in...");
    let account = SFAccount::login(
        env::var("SSO_USERNAME").expect("Missing SSO_USERNAME"),
        env::var("PASSWORD").expect("Missing PASSWORD")
    ).await.expect("Failed to login to S&F Account");

    let characters = account.characters().await.expect("Failed to fetch characters");
    let mut session = characters.into_iter()
        .flatten()
        .next()
        .expect("No character found on account!");

    println!("Connecting to game server: {}...", session.server_url());

    let _login_res = session.login().await.expect("Game login failed");

    // --- TEST LOGIC ---
    let target_player = "Spiritx";
    println!("Fetching guild info for: {}", target_player);

    let guild_result = get_guild_from_player(&mut session, target_player).await;

    println!("DEBUG: guild_result = {:?}", guild_result.unwrap().unwrap());
}
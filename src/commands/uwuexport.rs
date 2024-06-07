use std::collections::HashMap;

use serenity::{all::UserId, builder::CreateCommand};
use tracing::error;

use crate::Bot;

pub async fn run(bot: &Bot) -> String {
    // or integration perm
    // if !permissions.administrator() {
    //     "You must be an admin to run this command.".to_string()
    // }
    let leaderboard = bot.leaderboard.lock().await;

    match serde_json::to_string::<HashMap<UserId, usize>>(&leaderboard.scores) {
        Ok(leaderboard_export) => leaderboard_export.to_string(),
        Err(error) => {
            error!("{error:#}");
            "Error while parsing the leaderboard.".to_string()
        }
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("uwuexport").description("Export the leaderbord to the chat.")
}

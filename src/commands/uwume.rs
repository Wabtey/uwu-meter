use serenity::{all::UserId, builder::CreateCommand};

use crate::Bot;

pub async fn run(bot: &Bot, user_id: UserId) -> String {
    let leaderboard = bot.leaderboard.lock().await;
    let count = leaderboard.scores.get(&user_id).unwrap_or(&0);

    format!("<@{}>: {}\n", user_id, *count)
}

pub fn register() -> CreateCommand {
    CreateCommand::new("uwume").description("Display your UwU count")
}

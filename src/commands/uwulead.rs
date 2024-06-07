use serenity::builder::CreateCommand;

use crate::Bot;

pub async fn run(bot: &Bot) -> String {
    let leaderboard = bot.leaderboard.lock().await;
    let mut sorted_leaderboard: Vec<_> = leaderboard.scores.iter().collect();
    sorted_leaderboard.sort_by(|a, b| b.1.cmp(a.1));

    let mut response = String::from("UwU Leaderboard:\n");
    for (user_id, count) in sorted_leaderboard.iter().take(5) {
        response.push_str(&format!("<@{}>: {}\n", user_id, count));
    }
    response
}

pub fn register() -> CreateCommand {
    CreateCommand::new("uwulead").description("Display the UwU leaderboard")
}

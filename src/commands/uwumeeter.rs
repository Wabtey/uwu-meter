use std::sync::atomic::Ordering;

use serenity::builder::CreateCommand;

use crate::Bot;

pub async fn run(bot: &Bot) -> String {
    format!("UwU meter: {}", bot.uwu_count.load(Ordering::Relaxed))
}

pub fn register() -> CreateCommand {
    CreateCommand::new("uwumeeter").description("Display the total of UwU")
}

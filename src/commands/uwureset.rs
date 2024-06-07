use std::{collections::HashMap, sync::atomic::Ordering};

use serenity::{
    all::UserId,
    builder::{CreateCommand, CreateCommandOption},
    model::application::{ResolvedOption, ResolvedValue},
};

use crate::{Bot, Leaderboard};

pub async fn run(bot: &Bot, options: &[ResolvedOption<'_>]) -> String {
    // or integration perm
    // if !permissions.administrator() {
    //     "You must be an admin to run this command.".to_string()
    // }

    if let Some(ResolvedOption {
        value: ResolvedValue::String(leaderboard_string),
        ..
    }) = options.first()
    {
        match serde_json::from_str::<HashMap<UserId, usize>>(leaderboard_string) {
            Ok(new_leaderboard) => {
                // TODO: reset data
                let mut leaderboard = bot.leaderboard.lock().await;
                *leaderboard = Leaderboard {
                    scores: new_leaderboard,
                };

                let mut new_total = 0;
                for (_, uwu_user_count) in leaderboard.scores.iter().collect::<Vec<(_, &usize)>>() {
                    new_total += uwu_user_count;
                }

                bot.uwu_count.store(new_total, Ordering::Relaxed);

                /* ----------------------------- Save to persist ---------------------------- */
                let uwu_count = serde_json::to_string(&new_total).unwrap();
                bot.persist.save("uwu_count", uwu_count.as_bytes()).unwrap();

                let leaderboard_data = serde_json::to_string(&*leaderboard).unwrap();
                bot.persist
                    .save("leaderboard", leaderboard_data.as_bytes())
                    .unwrap();
                /* -------------------------------------------------------------------------- */
                format!("New Uwu meeter: {}", bot.uwu_count.load(Ordering::Relaxed))
            }
            Err(_) => "Error parsing the leaderboard. Nothing changed.".to_string(),
        }
    } else {
        "The leaderboard must be a string in the command's argument. Nothing changed.".to_string()
    }
}

pub fn register() -> CreateCommand {
    CreateCommand::new("uwureset")
        .description("Reset the current ladder with the given leaderbord (JSON)")
        .add_option(
            CreateCommandOption::new(
                // serenity::all::CommandOptionType::Attachment,
                serenity::all::CommandOptionType::String,
                "newleaderboard",
                "A JSON file. Must be in this structure: {\"1234\":4, \"5678\":2}",
                //{\"leaderboard\":{\"1234\":4, \"5678\":2},\"uwu_count\":6}
            )
            .required(true),
        )
}

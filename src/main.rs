use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serenity::all::{Command, CreateAllowedMentions, CreateCommandOption, UserId};
use serenity::{
    all::{Interaction, Message},
    async_trait,
    builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage},
    model::gateway::Ready,
    prelude::*,
};
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use tokio::sync::Mutex;
use tracing::info;

#[derive(Serialize, Deserialize)]
pub struct Leaderboard {
    scores: HashMap<UserId, usize>,
}
impl Leaderboard {
    fn new() -> Self {
        Leaderboard {
            scores: HashMap::new(),
        }
    }

    fn update_score(&mut self, user: UserId, score: usize) {
        *self.scores.entry(user).or_insert(0) += score;
    }
}

struct Bot {
    uwu_count: Arc<AtomicUsize>,
    leaderboard: Arc<Mutex<Leaderboard>>,
    persist: PersistInstance,
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands = vec![
            CreateCommand::new("uwumeeter").description("Display the total of UwU"),
            CreateCommand::new("uwulead").description("Display the UwU leaderboard"),
            CreateCommand::new("uwume").description("Display your UwU count"),
            CreateCommand::new("uwureset")
                .description("Reset the current ladder with the given leaderbord (JSON)")
                .add_option(CreateCommandOption::new(
                    // serenity::all::CommandOptionType::Attachment,
                    serenity::all::CommandOptionType::String,
                    "New Leaderboard",
                    "A JSON file. Must be in this structure: {\"1234\":4, \"5678\":2}",
                    //{\"leaderboard\":{\"1234\":4, \"5678\":2},\"uwu_count\":6}
                )),
        ];

        let commands = Command::set_global_commands(&ctx.http, commands)
            .await
            .unwrap();

        info!("Registered commands: {:#?}", commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let response_content = match command.data.name.as_str() {
                "uwumeeter" => format!("UwU meter: {}", self.uwu_count.load(Ordering::Relaxed)),
                "uwulead" => {
                    let leaderboard = self.leaderboard.lock().await;
                    let mut sorted_leaderboard: Vec<_> = leaderboard.scores.iter().collect();
                    sorted_leaderboard.sort_by(|a, b| b.1.cmp(a.1));

                    let mut response = String::from("UwU Leaderboard:\n");
                    for (user_id, count) in sorted_leaderboard.iter().take(10) {
                        response.push_str(&format!("<@{}>: {}\n", user_id, count));
                    }
                    response
                }
                "uwume" => {
                    let leaderboard = self.leaderboard.lock().await;
                    let user_id = command.user.id;
                    let count = leaderboard.scores.get(&user_id).unwrap_or(&0);

                    format!("<@{}>: {}\n", user_id, *count)
                }
                "uwureset" => {                
                    // or integration perm
                    // if !permissions.administrator() {
                    //     "You must be an admin to run this command.".to_string()
                    // }

                    let mut leaderboard = None;
                    for option in &command.data.options {
                        if option.name == "New Leaderboard" {
                            // let leaderboard = option.value.as_attachment_id();
                            leaderboard = option.value.as_str();
                        }
                    }

                    match leaderboard {
                        None => "The leaderboard must be a string in the command's argument. Nothing changed.".to_string(),
                        Some(leaderboard_string) => match serde_json::from_str::<HashMap<UserId, usize>>(leaderboard_string) {
                            Ok(new_leaderboard) => {
                                // TODO: reset data
                                let mut leaderboard = self.leaderboard.lock().await;
                                *leaderboard = Leaderboard {scores: new_leaderboard};

                                let mut new_total = 0;
                                for (_, uwu_user_count) in leaderboard.scores.iter().collect::<Vec<(_, &usize)>>() {
                                    new_total += uwu_user_count;
                                }
                                
                                self.uwu_count.store(new_total, Ordering::Relaxed);

                                /* ----------------------------- Save to persist ---------------------------- */
                                let uwu_count = serde_json::to_string(&new_total).unwrap();
                                self.persist
                                    .save("uwu_count", uwu_count.as_bytes())
                                    .unwrap();

                                let leaderboard_data = serde_json::to_string(&*leaderboard).unwrap();
                                self.persist
                                    .save("leaderboard", leaderboard_data.as_bytes())
                                    .unwrap();
                                /* -------------------------------------------------------------------------- */
                                format!(
                                    "New Uwu meeter: {}",
                                    self.uwu_count.load(Ordering::Relaxed)
                                )
                            }
                            Err(_) => {
                                "Error parsing the leaderboard. Nothing changed.".to_string()
                            }
                        }
                    }
                }
                command => unreachable!("Unknown command: {}", command),
            };

            let data = CreateInteractionResponseMessage::new()
                .content(response_content)
                // Avoid mention when tagging
                .allowed_mentions(CreateAllowedMentions::new());
            let builder = CreateInteractionResponse::Message(data);

            if let Err(why) = command.create_response(&ctx.http, builder).await {
                println!("Cannot respond to slash command: {why}");
            }
        }
    }

    async fn message(&self, _ctx: Context, msg: Message) {
        let content = msg.content.to_lowercase();
        if content.contains("uwu") && !msg.author.bot {
            /* ------------------------------- Update UwU ------------------------------- */
            self.uwu_count.fetch_add(1, Ordering::Relaxed);

            let mut leaderboard = self.leaderboard.lock().await;
            leaderboard.update_score(msg.author.id, 1);
            println!(
                "Leaderboard:\n{:#}",
                serde_json::to_string(&leaderboard.scores).unwrap()
            );

            /* -------------------------------- Save Uwu -------------------------------- */
            let uwu_count = serde_json::to_string(&self.uwu_count.load(Ordering::Relaxed)).unwrap();
            self.persist
                .save("uwu_count", uwu_count.as_bytes())
                .unwrap();

            let leaderboard_data = serde_json::to_string(&*leaderboard).unwrap();
            self.persist
                .save("leaderboard", leaderboard_data.as_bytes())
                .unwrap();
        }
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
    #[shuttle_persist::Persist] persist: PersistInstance,
) -> shuttle_serenity::ShuttleSerenity {
    /* ------------------------- Persistant Leaderboard ------------------------- */
    let leaderboard = if let Ok(data) = persist.load("leaderboard") {
        let data_str = String::from_utf8(data).unwrap();
        serde_json::from_str(&data_str).unwrap()
    } else {
        Leaderboard::new()
    };
    let uwu_count = if let Ok(data) = persist.load("uwu_count") {
        let data_str = String::from_utf8(data).unwrap();
        serde_json::from_str(&data_str).unwrap()
    } else {
        0
    };
    /* -------------------------------------------------------------------------- */

    // Get the discord token set in `Secrets.toml`
    let discord_token = secrets
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    // Set gateway intents, which decides what events the bot will be notified about.
    // Here we don't need any intents so empty
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let bot = Bot {
        uwu_count: Arc::new(AtomicUsize::new(uwu_count)),
        leaderboard: Arc::new(Mutex::new(leaderboard)),
        persist,
    };
    let client = Client::builder(discord_token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");

    Ok(client.into())
}

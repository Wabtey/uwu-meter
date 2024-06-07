use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serenity::{
    all::{Command, CreateAllowedMentions, Interaction, Message, UserId},
    async_trait,
    builder::{CreateInteractionResponse, CreateInteractionResponseMessage},
    model::gateway::Ready,
    prelude::*,
};
use shuttle_persist::PersistInstance;
use shuttle_runtime::SecretStore;
use tokio::sync::Mutex;
use tracing::info;

mod commands;

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
            commands::uwumeeter::register(),
            commands::uwulead::register(),
            commands::uwume::register(),
            commands::uwureset::register(),
        ];

        let global_commands = Command::set_global_commands(&ctx.http, commands).await;

        info!("Registered commands: {:#?}", global_commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let content = match command.data.name.as_str() {
                "uwumeeter" => Some(commands::uwumeeter::run(self).await),
                "uwulead" => Some(commands::uwulead::run(self).await),
                "uwume" => Some(commands::uwume::run(self, command.user.id).await),
                "uwureset" => Some(commands::uwureset::run(self, &command.data.options()).await),
                command => unreachable!("Unknown command: {}", command),
            };

            if let Some(content) = content {
                let data = CreateInteractionResponseMessage::new()
                    .content(content)
                    // Avoid mention when tagging
                    .allowed_mentions(CreateAllowedMentions::new());
                let builder = CreateInteractionResponse::Message(data);

                if let Err(why) = command.create_response(&ctx.http, builder).await {
                    println!("Cannot respond to slash command: {why}");
                }
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
            // info!(
            //     "Leaderboard:\n{:#}",
            //     serde_json::to_string(&leaderboard.scores).unwrap()
            // );

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

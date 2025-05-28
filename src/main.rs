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
use shuttle_runtime::SecretStore;
use sqlx::PgPool;
use tokio::sync::Mutex;
use tracing::{error, info};

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
    pool: PgPool,
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
            commands::uwuexport::register(),
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
                "uwuexport" => Some(commands::uwuexport::run(self).await),
                command => unreachable!("Unknown command: {}", command),
            };

            if let Some(content) = content {
                let data = CreateInteractionResponseMessage::new()
                    .content(content)
                    // Avoid mention when tagging
                    .allowed_mentions(CreateAllowedMentions::new());
                let builder = CreateInteractionResponse::Message(data);

                if let Err(why) = command.create_response(&ctx.http, builder).await {
                    error!("Cannot respond to slash command: {why}");
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
            let uwu_count = self.uwu_count.load(Ordering::Relaxed);
            sqlx::query!(
                "INSERT INTO uwu_data (key, value) VALUES ($1, $2) 
                 ON CONFLICT (key) DO UPDATE SET value = $2",
                "uwu_count",
                serde_json::to_string(&uwu_count).unwrap()
            )
            .execute(&self.pool)
            .await
            .unwrap();

            let leaderboard_data = serde_json::to_string(&*leaderboard).unwrap();
            sqlx::query!(
                "INSERT INTO uwu_data (key, value) VALUES ($1, $2) 
                 ON CONFLICT (key) DO UPDATE SET value = $2",
                "leaderboard",
                leaderboard_data
            )
            .execute(&self.pool)
            .await
            .unwrap();
        }
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> shuttle_serenity::ShuttleSerenity {
    /* ----------------------------- Leaderboard DB ----------------------------- */
    let leaderboard = if let Ok(row) =
        sqlx::query!("SELECT value FROM uwu_data WHERE key = $1", "leaderboard")
            .fetch_one(&pool)
            .await
    {
        serde_json::from_str(&row.value).unwrap()
    } else {
        Leaderboard::new()
    };

    let uwu_count = if let Ok(row) =
        sqlx::query!("SELECT value FROM uwu_data WHERE key = $1", "uwu_count")
            .fetch_one(&pool)
            .await
    {
        serde_json::from_str(&row.value).unwrap()
    } else {
        0
    };

    /* -------------------------DISCORD_TOKEN------------------------------------------------- */
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
        pool,
    };

    let client = Client::builder(discord_token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");

    Ok(client.into())
}

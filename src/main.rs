use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use anyhow::Context as _;
use serenity::all::{Command, CreateAllowedMentions};
use serenity::{
    all::{Interaction, Message},
    async_trait,
    builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage},
    model::gateway::Ready,
    prelude::*,
};
use shuttle_runtime::SecretStore;
use tokio::{fs, io::AsyncWriteExt, sync::Mutex};
use tracing::info;

struct Bot {
    uwu_count: Arc<AtomicUsize>,
    leaderboard: Arc<Mutex<HashMap<String, usize>>>,
}

impl Bot {
    async fn save_data(&self) -> anyhow::Result<()> {
        let uwu_count = self.uwu_count.load(Ordering::Relaxed);
        let leaderboard = self.leaderboard.lock().await;

        let data = serde_json::json!({
            "uwu_count": uwu_count,
            "leaderboard": *leaderboard
        });

        let mut file = fs::File::create("data/uwu.json").await?;
        file.write_all(data.to_string().as_bytes()).await?;
        Ok(())
    }

    async fn load_data(&self) -> anyhow::Result<()> {
        let data = fs::read_to_string("data/uwu.json")
            .await
            .unwrap_or_else(|_| "{}".to_string());
        let json: serde_json::Value = serde_json::from_str(&data)?;

        if let Some(count) = json["uwu_count"].as_u64() {
            self.uwu_count.store(count as usize, Ordering::Relaxed);
        }

        if let Some(leaderboard) = json["leaderboard"].as_object() {
            let mut leaderboard_lock = self.leaderboard.lock().await;
            for (key, value) in leaderboard {
                if let Some(count) = value.as_u64() {
                    leaderboard_lock.insert(key.clone(), count as usize);
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands = vec![
            CreateCommand::new("uwumeeter").description("Display the number of UwU pronounced"),
            CreateCommand::new("uwulead").description("Display the UwU leaderboard"),
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
                    let mut sorted_leaderboard: Vec<_> = leaderboard.iter().collect();
                    sorted_leaderboard.sort_by(|a, b| b.1.cmp(a.1));

                    let mut response = String::from("UwU Leaderboard:\n");
                    for (user_id, count) in sorted_leaderboard.iter().take(10) {
                        response.push_str(&format!("<@{}>: {}\n", user_id, count));
                    }
                    response
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
            self.uwu_count.fetch_add(1, Ordering::Relaxed);
            {
                let mut leaderboard = self.leaderboard.lock().await;
                *leaderboard.entry(msg.author.id.to_string()).or_insert(0) += 1;
            }
            self.save_data().await.expect("Failed to save data");
        }
    }
}

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let discord_token = secrets
        .get("DISCORD_TOKEN")
        .context("'DISCORD_TOKEN' was not found")?;

    let client = get_client(&discord_token).await;
    Ok(client.into())
}

pub async fn get_client(discord_token: &str) -> Client {
    // Set gateway intents, which decides what events the bot will be notified about.
    // Here we don't need any intents so empty
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let bot = Bot {
        uwu_count: Arc::new(AtomicUsize::new(0)),
        leaderboard: Arc::new(Mutex::new(HashMap::new())),
    };

    bot.load_data().await.expect("Failed to load data");

    Client::builder(discord_token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client")
}

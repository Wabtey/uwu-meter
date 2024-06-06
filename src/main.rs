use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use anyhow::Context as _;
use serenity::{
    all::{GuildId, Interaction, Message},
    async_trait,
    builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage},
    model::gateway::Ready,
    prelude::*,
};
use shuttle_runtime::SecretStore;
use tracing::info;

struct Bot {
    discord_guild_id: GuildId,
    uwu_count: Arc<AtomicUsize>,
    leaderboard: Arc<Mutex<HashMap<String, usize>>>,
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let commands = vec![
            // CreateCommand::new("uwu").description("Say uwu"),
            CreateCommand::new("uwumeeter").description("Display the number of UwU pronounced"),
            CreateCommand::new("uwulead").description("Display the UwU leaderboard"),
        ];

        let commands = &self
            .discord_guild_id
            .set_commands(&ctx.http, commands)
            .await
            .unwrap();

        info!("Registered commands: {:#?}", commands);
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let response_content = match command.data.name.as_str() {
                // "uwu" => "uwu".to_owned(),
                "uwumeeter" => format!("UwU meter: {}", self.uwu_count.load(Ordering::Relaxed)),
                "uwulead" => {
                    let leaderboard = self.leaderboard.lock().unwrap();
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

            let data = CreateInteractionResponseMessage::new().content(response_content);
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
            let mut leaderboard = self.leaderboard.lock().unwrap();
            *leaderboard.entry(msg.author.id.to_string()).or_insert(0) += 1;
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

    let discord_guild_id = secrets
        .get("DISCORD_GUILD_ID")
        .context("'DISCORD_GUILD_ID' was not found")?;

    let client = get_client(&discord_token, discord_guild_id.parse().unwrap()).await;
    Ok(client.into())
}

pub async fn get_client(discord_token: &str, discord_guild_id: u64) -> Client {
    // Set gateway intents, which decides what events the bot will be notified about.
    // Here we don't need any intents so empty
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let uwu_count = Arc::new(AtomicUsize::new(0));
    let leaderboard = Arc::new(Mutex::new(HashMap::new()));

    Client::builder(discord_token, intents)
        .event_handler(Bot {
            discord_guild_id: GuildId::new(discord_guild_id),
            uwu_count: Arc::clone(&uwu_count),
            leaderboard: Arc::clone(&leaderboard),
        })
        .await
        .expect("Err creating client")
}

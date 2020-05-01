#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate serde;
#[macro_use]
extern crate log;
extern crate log4rs;

use chrono::{DateTime, Utc};
use clap::{App, Arg};
use log::LevelFilter;
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use serenity::client::Client;
use serenity::framework::standard::macros::{group, help};
use serenity::framework::standard::{help_commands, Args, CommandGroup, HelpOptions};
use serenity::framework::standard::{CommandResult, StandardFramework};
use serenity::model::channel::{Message, Reaction};
use serenity::model::id::UserId;
use serenity::model::prelude::Ready;
use serenity::model::user::User;
use serenity::prelude::TypeMapKey;
use serenity::prelude::{Context, EventHandler, RwLock, ShareMap};
use serenity::CacheAndHttp;
use std::collections::HashSet;
use std::process::exit;
use std::sync::Arc;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

mod database;
mod discord;
mod hypebot_config;

use database::models::NewEvent;
use database::*;
use discord::events::{CANCEL_COMMAND, CONFIRM_COMMAND, CREATE_COMMAND};
use discord::{
    get_config, log_error, permission_check, send_dm_message, send_message_to_reaction_users,
};
use hypebot_config::HypeBotConfig;

const INTERESTED_EMOJI: &str = "\u{2705}";
const UNINTERESTED_EMOJI: &str = "\u{274C}";

type HypeBotResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Event discord group
#[group]
#[only_in(guilds)]
#[description("Commands for Creating Events")]
#[commands(create, confirm, cancel)]
struct EventCommands;

/// Struct for storing drafted events
struct DraftEvent {
    pub event: NewEvent,
    pub creator_id: u64,
}

impl TypeMapKey for DraftEvent {
    type Value = DraftEvent;
}

/// Handler for Discord events
struct Handler;

impl EventHandler for Handler {
    /// On reaction add
    fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        if reaction.emoji.as_data() == INTERESTED_EMOJI {
            send_message_to_reaction_users(
                &ctx,
                &reaction,
                "Hello, you are now receiving reminders for **{event}**",
            );
        }
    }

    /// On reaction remove
    fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        if reaction.emoji.as_data() == INTERESTED_EMOJI {
            send_message_to_reaction_users(
                &ctx,
                &reaction,
                "Hello, you are no longer receiving reminders for **{event}**",
            );
        }
    }

    /// On bot ready
    fn ready(&self, _: Context, ready: Ready) {
        println!("Connected to Discord as {}", ready.user.name);
        info!("Connected to Discord as {}", ready.user.name);
    }
}

#[help]
#[command_not_found_text = "Could not find: `{}`."]
#[strikethrough_commands_tip_in_guild("HypeBot")]
fn bot_help(
    context: &mut Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    help_commands::with_embeds(context, msg, args, help_options, groups, owners)
}

/// Thread to send reminders to users
fn send_reminders(cache_and_http: &Arc<CacheAndHttp>, data: &Arc<RwLock<ShareMap>>) {
    let sleep_duration = Duration::from_secs(60);
    let config = get_config(data).unwrap();
    loop {
        sleep(sleep_duration);
        let http = &cache_and_http.http;
        let event_channel_id = config.event_channel;

        // Get all current events
        if let Ok(events) = get_all_events(config.db_url.clone()) {
            for event in events {
                if let Ok(message_id) = event.message_id.parse::<u64>() {
                    // Get time to event
                    let utc_time = DateTime::<Utc>::from_utc(event.event_time.clone(), Utc);
                    let current_time = chrono::offset::Utc::now();
                    let time_to_event = (utc_time - current_time).num_minutes();
                    // If the event starts in less than 10 minutes
                    if time_to_event <= 10 && time_to_event > 0 && event.reminder_sent < 1 {
                        // Get message id
                        if let Ok(message) = http.get_message(event_channel_id, message_id) {
                            let reaction_users = message
                                .reaction_users(&http, INTERESTED_EMOJI, None, None)
                                .unwrap_or(Vec::<User>::new());

                            // Build reminder message
                            let msg: String = format!(
                                "Hello! **{}** begins in **{} minutes**!",
                                &event.event_name, time_to_event
                            );

                            // Send reminder to each reacted user
                            for user in reaction_users {
                                send_dm_message(&http, user, &msg);
                            }
                        }

                        set_reminder(config.db_url.clone(), event.id, 1).ok();
                    } else if time_to_event < -60 {
                        remove_event(config.db_url.clone(), event.id).ok();
                        http.delete_message(event_channel_id, message_id).ok();
                    }
                }
            }
        }
    }
}

/// Does the setup for logging
fn setup_logging(config: &HypeBotConfig) -> HypeBotResult<()> {
    // Setup logging
    let log_encode = Box::new(PatternEncoder::new("{d}:{l}-{m}\n"));

    let logfile = FileAppender::builder()
        .encoder(log_encode)
        .build(&config.log_path)?;

    let log_config = Config::builder()
        .appender(Appender::builder().build("logfile", Box::new(logfile)))
        .build(Root::builder().appender("logfile").build(LevelFilter::Info))?;

    log4rs::init_config(log_config)?;

    Ok(())
}

embed_migrations!("migrations/");
fn main() -> HypeBotResult<()> {
    // Initialize arg parser
    let mut app = App::new("Hype Bot")
        .about("Hype Bot: Hype Up Your Discord Events!")
        .arg(
            Arg::with_name("config")
                .index(1)
                .short("c")
                .long("config")
                .value_name("CONFIG_PATH")
                .help("Config file path"),
        );

    // Get arg parser
    let matches = app.clone().get_matches();

    // Check if config is set
    if let Some(config_path) = matches.value_of("config") {
        // Load config
        let cfg = match hypebot_config::HypeBotConfig::new(config_path) {
            Ok(cfg) => cfg,
            Err(err) => {
                println!("Error opening config file: {}", err);
                exit(-1);
            }
        };

        // Setup logging
        setup_logging(&cfg)?;

        // Run migrations
        let connection = establish_connection(cfg.db_url.clone());
        embedded_migrations::run(&connection)?;

        // New client
        let mut client =
            Client::new(cfg.discord_key.clone(), Handler).expect("Error creating client");

        // Configure client
        client.with_framework(
            StandardFramework::new()
                .configure(|c| {
                    c.prefix(cfg.prefix.as_str().clone())
                        .allow_dm(false)
                        .ignore_bots(true)
                        .ignore_webhooks(true)
                })
                .before(permission_check)
                .after(log_error)
                .group(&EVENTCOMMANDS_GROUP)
                .help(&BOT_HELP),
        );

        // Copy config data to client data
        {
            let mut data = client.data.write();
            data.insert::<HypeBotConfig>(cfg);
            data.insert::<DraftEvent>(DraftEvent {
                event: NewEvent {
                    message_id: String::new(),
                    event_time: Utc::now().naive_utc(),
                    event_name: String::new(),
                    event_desc: String::new(),
                    thumbnail_link: String::new(),
                    reminder_sent: 0 as i32,
                },
                creator_id: 0,
            });
        }
        let data = client.data.clone();
        let cache_and_http = client.cache_and_http.clone();
        thread::spawn(move || send_reminders(&cache_and_http, &data));

        // Start bot
        info!("Starting HypeBot!");
        println!("Starting HypeBot!");
        if let Err(why) = client.start() {
            println!("An error occurred while running the client: {:?}", why);
        }
    } else {
        // Print help
        app.print_help()?;
    }

    Ok(())
}

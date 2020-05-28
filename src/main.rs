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
use log4rs::append::rolling_file::{RollingFileAppender};
use log4rs::config::{Appender, Config, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::append::console::ConsoleAppender;
use log4rs::append::rolling_file::policy::compound::roll::fixed_window::FixedWindowRoller;
use log4rs::append::rolling_file::policy::compound::trigger::size::SizeTrigger;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::filter::threshold::ThresholdFilter;
use log4rs::init_config;
use serenity::client::Client;
use serenity::framework::standard::macros::{group, help};
use serenity::framework::standard::{help_commands, Args, CommandGroup, HelpOptions};
use serenity::framework::standard::{CommandResult, StandardFramework};
use serenity::model::channel::{Message, Reaction};
use serenity::model::id::UserId;
use serenity::model::prelude::Ready;
use serenity::prelude::{Context, EventHandler, RwLock};
use std::collections::HashSet;
use std::process::exit;
use std::sync::Arc;
use white_rabbit::{DateResult, Scheduler};
use std::path::Path;

mod database;
mod discord;
mod hypebot_config;

use database::models::NewEvent;
use database::*;
use discord::events::{CANCEL_COMMAND, CONFIRM_COMMAND, CREATE_COMMAND};
use discord::{
    delete_event, get_config, get_scheduler, log_error, permission_check,
    send_message_to_reaction_users, schedule_event, DraftEvent, SchedulerKey,
};
use hypebot_config::HypeBotConfig;

const INTERESTED_EMOJI: &str = "\u{2705}";
const UNINTERESTED_EMOJI: &str = "\u{274C}";

type HypeBotResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Event command group
#[group]
#[only_in(guilds)]
#[description("Commands for Creating Events")]
#[commands(create, confirm, cancel)]
struct EventCommands;

/// Handler for Discord events
struct Handler;

impl EventHandler for Handler {
    /// On reaction add
    fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        let config = match get_config(&ctx.data) {
            Ok(config) => config,
            Err(e) => {
                error!("Unable to get config: {}", e.0);
                return;
            }
        };
        if reaction.channel_id.0 == config.event_channel && reaction.emoji.as_data() == INTERESTED_EMOJI {
            send_message_to_reaction_users(
                &ctx,
                &reaction,
                "Hello, you are now receiving reminders for **{event}**",
            );
        }
    }

    /// On reaction remove
    fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        let config = match get_config(&ctx.data) {
            Ok(config) => config,
            Err(e) => {
                error!("Unable to get config: {}", e.0);
                return;
            }
        };
        if reaction.channel_id.0 == config.event_channel && reaction.emoji.as_data() == INTERESTED_EMOJI {
            send_message_to_reaction_users(
                &ctx,
                &reaction,
                "Hello, you are no longer receiving reminders for **{event}**",
            );
        }
    }

    /// On bot ready
    fn ready(&self, _: Context, ready: Ready) {
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

/// Does the setup for logging
fn setup_logging(config: &HypeBotConfig) -> HypeBotResult<()> {
    // Build log file path
    let log_file_path = Path::new(&config.log_path);
    let log_file_path = log_file_path.join("hype_bot.log");
    let archive = log_file_path.join("hype_bot.{}.log");

    // Number of logs to keep
    let window_size = 10;

    // 10MB file size limit
    let size_limit = 10 * 1024 * 1024;
    let size_trigger = SizeTrigger::new(size_limit);

    let fixed_window_roller = FixedWindowRoller::builder()
        .build(archive.to_str().unwrap(), window_size)
        .unwrap();

    let compound_policy =
        CompoundPolicy::new(Box::new(size_trigger), Box::new(fixed_window_roller));

    let config = Config::builder()
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(LevelFilter::Info)))
                .build(
                    "logfile",
                    Box::new(
                        RollingFileAppender::builder()
                            .encoder(Box::new(PatternEncoder::new("{d} {l}::{m}{n}")))
                            .build(log_file_path, Box::new(compound_policy))?,
                    ),
                ),
        )
        .appender(
            Appender::builder()
                .filter(Box::new(ThresholdFilter::new(LevelFilter::Info)))
                .build(
                    "stdout",
                    Box::new(
                        ConsoleAppender::builder()
                            .encoder(Box::new(PatternEncoder::new("{l}::{m}{n}")))
                            .build(),
                    ),
                ),
        )
        .build(
            Root::builder()
                .appender("logfile")
                .appender("stdout")
                .build(LevelFilter::Info),
        )?;

    init_config(config)?;

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

        // Copy config data to client data and setup scheduler
        {
            let mut data = client.data.write();
            data.insert::<HypeBotConfig>(cfg);
            data.insert::<DraftEvent>(DraftEvent {
                event: NewEvent {
                    message_id: String::new(),
                    event_time: Utc::now().naive_utc(),
                    event_name: String::new(),
                    organizer: String::new(),
                    event_desc: String::new(),
                    event_loc: String::new(),
                    thumbnail_link: String::new(),
                    reminder_sent: 0 as i32,
                },
                creator_id: 0,
            });

            // Create scheduler
            let scheduler = Scheduler::new(2);
            let scheduler = Arc::new(RwLock::new(scheduler));
            data.insert::<SchedulerKey>(scheduler);
        }

        // Schedule current events
        let config = get_config(&client.data).expect("Unable to find get config");
        let duration = chrono::Duration::minutes(60);
        for event in get_all_events(config.db_url.clone()).unwrap() {
            let event_time: DateTime<Utc> =
                DateTime::<Utc>::from_utc(event.event_time.clone(), Utc);

            if Utc::now() > event_time + duration {
                delete_event(&client.cache_and_http.http, &client.data, &event);
            } else if Utc::now() > event_time {
                let scheduler = get_scheduler(&client.data).unwrap();
                let mut scheduler = scheduler.write();
                let cancel_time = event_time + duration;
                let http = client.cache_and_http.http.clone();
                let data = client.data.clone();

                scheduler.add_task_datetime(cancel_time, move |_| {
                    delete_event(&http, &data, &event);
                    DateResult::Done
                });
            } else if event.reminder_sent == 0 {
                schedule_event(&client.cache_and_http.http, &client.data, &event);
            }
        }

        // Start bot
        info!("Starting HypeBot!");
        if let Err(why) = client.start() {
            error!("An error occurred while running the client: {:?}", why);
        }
    } else {
        // Print help
        app.print_help()?;
    }

    Ok(())
}

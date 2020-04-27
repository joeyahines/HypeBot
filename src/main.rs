#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate serde;

use chrono::{DateTime, Datelike, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use clap::{App, Arg};
use serenity::client::Client;
use serenity::framework::standard::macros::{command, group};
use serenity::framework::standard::Args;
use serenity::framework::standard::{CommandError, CommandResult, StandardFramework};
use serenity::http::Http;
use serenity::model::channel::{Message, Reaction};
use serenity::model::prelude::{ChannelId, Ready};
use serenity::prelude::TypeMapKey;
use serenity::prelude::{Context, EventHandler, RwLock, ShareMap};
use serenity::utils::{content_safe, Colour, ContentSafeOptions};
use serenity::CacheAndHttp;
use serenity::Result;
use std::process::exit;
use std::sync::Arc;
use std::thread;
use std::thread::sleep;
use std::time::Duration;

mod database;
mod hypebot_config;

use crate::database::models::NewEvent;
use database::*;
use hypebot_config::HypeBotConfig;
use serenity::model::user::User;

const INTERESTED_EMOJI: &str = "\u{2705}";
const UNINTERESTED_EMOJI: &str = "\u{274C}";

/// Event commands group
#[group]
#[commands(create_event, confirm_event, cancel_event)]
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
                "Hello, you are now receiving reminders for",
            );
        }
    }

    /// On reaction remove
    fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        if reaction.emoji.as_data() == INTERESTED_EMOJI {
            send_message_to_reaction_users(
                &ctx,
                &reaction,
                "Hello, you are no longer receiving reminders for",
            );
        }
    }

    /// On bot ready
    fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
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
                    let time_to_event = (utc_time - chrono::offset::Utc::now()).num_minutes();
                    // If the event starts in less than 10 minutes
                    if time_to_event <= 10 && time_to_event > 0 && event.reminder_sent == 1 {
                        // Get message isd
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

/// Send a message to a reaction user
///
/// Message will be sent in the format
/// ```
/// "{msg_text} **event_name**"
/// ```
fn send_message_to_reaction_users(ctx: &Context, reaction: &Reaction, msg_text: &str) {
    if let Ok(config) = get_config(&ctx.data) {
        let db_link = config.db_url.clone();
        let message_id = reaction.message_id.0.to_string();

        let event = match get_event_by_msg_id(db_link, message_id) {
            Ok(event) => event,
            Err(_) => {
                return;
            }
        };

        // Format message
        let msg: String = format!("{} **{}**", msg_text, event.event_name);

        if let Ok(user) = reaction.user(&ctx.http) {
            send_dm_message(&ctx.http, user, &msg);
        }
    }
}

/// Send a DM message to a user
fn send_dm_message(http: &Http, user: User, message: &String) {
    if let Ok(dm_channel) = user.create_dm_channel(&http) {
        dm_channel.send_message(&http, |m| m.content(message)).ok();
    }
}

/// Sends the event message to the event channel
fn send_event_msg(
    http: &Http,
    config: &HypeBotConfig,
    channel_id: u64,
    event: &NewEvent,
    react: bool,
) -> Result<Message> {
    let channel = http.get_channel(channel_id)?;

    let utc_time = DateTime::<Utc>::from_utc(event.event_time.clone(), Utc);

    let native_time = utc_time.with_timezone(&config.event_timezone);

    // Send message
    let msg = channel.id().send_message(&http, |m| {
        m.embed(|e| {
            e.title(event.event_name.clone())
                .color(Colour::PURPLE)
                .description(format!(
                    "**{}**\n{}",
                    native_time.format("%A, %B %d @ %I:%M %P %t %Z"),
                    event.event_desc
                ))
                .thumbnail(event.thumbnail_link.clone())
                .footer(|f| f.text("Local Event Time"))
                .timestamp(utc_time.to_rfc3339())
        })
    })?;

    if react {
        // Add reacts
        msg.react(http, INTERESTED_EMOJI)?;
        msg.react(http, UNINTERESTED_EMOJI)?;
    }

    Ok(msg)
}

/// Updates the draft event stored in the context data
fn update_draft_event(
    ctx: &Context,
    event_name: String,
    event_desc: String,
    thumbnail: String,
    event_time: NaiveDateTime,
    creator_id: u64,
) -> CommandResult {
    let mut data = ctx.data.write();
    let mut draft_event = data
        .get_mut::<DraftEvent>()
        .ok_or(CommandError("Unable get draft event!".to_string()))?;

    draft_event.event.event_name = event_name;
    draft_event.event.event_desc = event_desc;
    draft_event.event.thumbnail_link = thumbnail;
    draft_event.event.message_id = String::new();
    draft_event.event.event_time = event_time;
    draft_event.creator_id = creator_id;
    Ok(())
}

/// Sends the draft event stored in the context data
fn send_draft_event(ctx: &Context, channel: ChannelId) -> CommandResult {
    let data = ctx.data.read();
    let config = data
        .get::<HypeBotConfig>()
        .ok_or(CommandError("Config not found!".to_string()))?;
    let draft_event = data
        .get::<DraftEvent>()
        .ok_or(CommandError("Draft event not found!".to_string()))?;

    channel.send_message(&ctx, |m| {
        m.content(format!(
            "Draft message, use the `confirm_event` command to post it."
        ))
    })?;
    send_event_msg(&ctx.http, config, channel.0, &draft_event.event, false)?;
    Ok(())
}

/// Gets the config from context data
fn get_config(data: &Arc<RwLock<ShareMap>>) -> std::result::Result<HypeBotConfig, CommandError> {
    let data_read = data.read();
    let config = data_read
        .get::<HypeBotConfig>()
        .ok_or(CommandError("Unable to get config".to_string()))?;

    Ok(config.clone())
}

/// Checks if the user has permission to use this bot
fn permission_check(ctx: &mut Context, msg: &Message, _command_name: &str) -> bool {
    if let Some(guild_id) = msg.guild_id {
        if let Ok(config) = get_config(&ctx.data) {
            if let Ok(roles) = ctx.http.get_guild_roles(guild_id.0) {
                for role in roles {
                    if config.event_roles.contains(&role.id.0) {
                        return match msg.author.has_role(ctx, guild_id, role) {
                            Ok(has_role) => has_role,
                            Err(_) => false,
                        };
                    }
                }
            }
        }
    }

    false
}

#[command]
/// Posts the pending event in the shared context
fn confirm_event(ctx: &mut Context, msg: &Message, _args: Args) -> CommandResult {
    let config = get_config(&ctx.data)?;
    let data = ctx.data.read();

    // Get draft event
    if let Some(draft_event) = data.get::<DraftEvent>() {
        let new_event = &draft_event.event;
        // Check to to see if message author is the owner of the pending event
        if draft_event.creator_id == msg.author.id.0 {
            // Send event message
            let event_msg =
                send_event_msg(&ctx.http, &config, config.event_channel, new_event, true)?;

            msg.reply(&ctx, "Event posted!")?;

            let new_event = NewEvent {
                message_id: event_msg.id.0.to_string(),
                event_time: new_event.event_time.clone(),
                event_desc: new_event.event_desc.clone(),
                event_name: new_event.event_name.clone(),
                thumbnail_link: new_event.event_name.clone(),
                reminder_sent: 0,
            };

            insert_event(config.db_url.clone(), &new_event).ok();
        } else {
            msg.reply(&ctx, format!("You do not have a pending event!"))?;
        }
    } else {
        msg.reply(&ctx, format!("There are no pending events!!"))?;
    }

    Ok(())
}

#[command]
/// Creates an event and announce it
fn create_event(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Get config
    let config = get_config(&ctx.data)?;
    let guild_id = msg
        .guild_id
        .ok_or(CommandError("Unable to get guild ID".to_string()))?;

    // Parse args
    let event_name = args.single::<String>()?.replace("\"", "");
    let date_string = args.single::<String>()?.replace("\"", "");
    let description = args.single::<String>()?.replace("\"", "");
    let thumbnail_link = match args.single::<String>() {
        Ok(link) => link.replace("<", "").replace(">", ""),
        Err(_) => config.default_thumbnail_link.clone(),
    };

    // Parse date
    let tz: Tz = config.event_timezone;
    let input_date = NaiveDateTime::parse_from_str(date_string.as_str(), "%I:%M%p %Y-%m-%d")?;

    let input_date = tz
        .ymd(
            input_date.date().year(),
            input_date.date().month(),
            input_date.date().day(),
        )
        .and_hms(
            input_date.time().hour(),
            input_date.time().minute(),
            input_date.time().second(),
        );

    let event_time = input_date.with_timezone(&Utc).naive_utc();

    // Clean channel, role, and everyone pings
    let settings = ContentSafeOptions::default()
        .clean_role(true)
        .clean_here(true)
        .clean_user(true)
        .clean_everyone(true)
        .display_as_member_from(guild_id);

    let description = content_safe(&ctx.cache, description, &settings);
    let event_name = content_safe(&ctx.cache, event_name, &settings);

    update_draft_event(
        &ctx,
        event_name,
        description,
        thumbnail_link,
        event_time,
        msg.author.id.0,
    )?;
    send_draft_event(&ctx, msg.channel_id)?;

    Ok(())
}

#[command]
/// Cancels an event
fn cancel_event(ctx: &mut Context, _msg: &Message, mut args: Args) -> CommandResult {
    let config = get_config(&ctx.data)?;

    // Parse args
    let event_name = args.single::<String>()?.replace("\"", "");

    let event = get_event_by_name(config.db_url.clone(), event_name)?;
    let message_id = event.message_id.parse::<u64>()?;
    let message = ctx.http.get_message(config.event_channel, message_id)?;

    let reaction_users = message
        .reaction_users(&ctx.http, INTERESTED_EMOJI, None, None)
        .unwrap_or(Vec::<User>::new());

    let string = &format!("**{}** has been canceled!", event.event_name.clone());

    for user in reaction_users {
        send_dm_message(&ctx.http, user, &string);
    }

    remove_event(config.db_url.clone(), event.id)?;

    message.delete(&ctx)?;

    Ok(())
}

embed_migrations!("migrations/");
fn main() -> clap::Result<()> {
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

        // Run migrations
        let connection = establish_connection(cfg.db_url.clone());
        embedded_migrations::run(&connection).expect("Unable to run migrations");

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
                .group(&EVENTCOMMANDS_GROUP),
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
        println!("Starting Hypebot!");
        if let Err(why) = client.start() {
            println!("An error occurred while running the client: {:?}", why);
        }
    } else {
        // Print help
        app.print_help()?;
    }

    Ok(())
}

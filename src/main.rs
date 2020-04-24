#[macro_use]
extern crate diesel;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate diesel_migrations;
extern crate serde;

use serenity::client::Client;
use serenity::model::channel::{Message};
use serenity::prelude::{EventHandler, Context};
use serenity::utils::{content_safe, ContentSafeOptions, Colour};
use serenity::model::gateway::Ready;
use serenity::framework::standard::StandardFramework;
use serenity::framework::standard::CommandResult;
use serenity::framework::standard::macros::{command, group,};
use serenity::framework::standard::Args;
use serenity::prelude::TypeMapKey;
use serenity::model::error::Error;
use clap::{Arg, App};
use chrono::{DateTime, Utc, NaiveDateTime, Datelike, Timelike, TimeZone};
use chrono_tz::Tz;
use std::process::exit;

mod hypebot_config;
mod database;

use database::*;
use hypebot_config::HypeBotConfig;
use crate::database::models::NewEvent;

/// Event commands group
#[group]
#[commands(create_event, confirm_event)]
struct EventCommands;

/// Handler for Discord events
struct Handler;

impl EventHandler for Handler {
    /// On bot ready
    fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
}

/// Struct for storing drafted events
struct DraftEvent {
    pub event: NewEvent,
    pub creator_id: u64
}

impl TypeMapKey for DraftEvent {
    type Value = DraftEvent;
}

embed_migrations!("migrations/");
fn main() {
    // Initialize arg parser
    let mut app = App::new("Hype Bot")
        .about("Hype Bot: Hype Up Your Discord Events!").arg(Arg::with_name("config")
            .index(1)
            .short("c").
            long("config")
            .value_name("CONFIG_PATH")
            .help("Config file path"));

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
        let mut client = Client::new(cfg.discord_key.clone(), Handler)
            .expect("Error creating client");

        // Configure client
        client.with_framework(StandardFramework::new()
            .configure(|c| c.prefix(cfg.prefix.as_str().clone()))
            .group(&EVENTCOMMANDS_GROUP));

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
                },
                creator_id: 0
            });
        }

        // Start bot
        println!("Starting Hypebot!");
        if let Err(why) = client.start() {
            println!("An error occurred while running the client: {:?}", why);
        }
    }
    else {
        // Print help
        app.print_help().unwrap();
    }
}

fn send_event_msg(ctx: &Context, channel_id: u64, event: &NewEvent) -> CommandResult {
    let data = ctx.data.read();
    let config = data.get::<HypeBotConfig>().unwrap();
    let channel = ctx.http.get_channel(channel_id)?;

    let tz: Tz = config.event_timezone.parse()?;
    let utc_time = DateTime::<Utc>::from_utc(event.event_time.clone(), Utc);
    let native_time = utc_time.with_timezone(&tz);

    // Send message
    let msg = channel.id().send_message(&ctx, |m| {
        m.embed(|e| {
            e.title(event.event_name.clone())
                .color(Colour::PURPLE)
                .description(format!("**{}**\n{}", native_time.format("%A, %B %d @ %I:%M %P %t %Z"), event.event_desc))
                .thumbnail(event.thumbnail_link.clone())
                .footer(|f| { f.text("Local Event Time") })
                .timestamp(utc_time.to_rfc3339())
        })
    })?;

    // Add reacts
    msg.react(ctx, "\u{2705}")?;
    msg.react(ctx, "\u{274C}")?;

    Ok(())
}

#[command]
fn confirm_event(ctx: &mut Context, msg: &Message, _args: Args) -> CommandResult {
    let data = ctx.data.read();
    let config = data.get::<HypeBotConfig>().unwrap();
    let draft_event = match data.get::<DraftEvent>() {
        Some(draft_event) => Ok(draft_event),
        None => Err(Error::ItemMissing)
    }?;
    let new_event = &draft_event.event;

    if draft_event.creator_id == msg.author.id.0 {
        send_event_msg(ctx,  config.event_channel, new_event)?;

        // Insert event into the database
        insert_event(config.db_url.clone(), new_event);

        msg.reply(&ctx, "Event posted!")?;
    }
    else {
        msg.reply(&ctx, format!("You do not have a pending event!"))?;
    }

    Ok(())
}

#[command]
/// Creates an event and announce it
fn create_event (ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    let mut event_name;
    let mut description;
    let thumbnail_link;
    let date;

    {
        // Open config
        let data = ctx.data.read();
        let config = data.get::<HypeBotConfig>().unwrap();

        // Parse args
        event_name = args.single::<String>()?.replace("\"", "");
        let date_string = args.single::<String>()?.replace("\"", "");
        description = args.single::<String>()?.replace("\"", "");
        thumbnail_link = match args.single::<String>() {
            Ok(link) => link.replace("<", "").replace(">", ""),
            Err(_) => config.default_thumbnail_link.clone(),
        };

        // Parse date
        let tz: Tz = config.event_timezone.parse()?;
        let input_date = NaiveDateTime::parse_from_str(date_string.as_str(), "%I:%M%p %Y-%m-%d")?;
        let input_date = tz.ymd(input_date.date().year(), input_date.date().month(), input_date.date().day())
            .and_hms(input_date.time().hour(), input_date.time().minute(), input_date.time().second());
        date = input_date.with_timezone(&Utc);

        // Clean channel, role, and everyone pings
        let settings = ContentSafeOptions::default()
            .clean_role(true)
            .clean_here(true)
            .clean_user(true)
            .clean_everyone(true)
            .display_as_member_from(msg.guild_id.unwrap());

        description = content_safe(&ctx.cache, description, &settings);
        event_name = content_safe(&ctx.cache, event_name, &settings);
    }

    {
        let mut data = ctx.data.write();
        let mut draft_event = match data.get_mut::<DraftEvent>() {
            Some(event) => event,
            None => {
                println!("Error");
                panic!("Can't get write lock")
            }
        };
        draft_event.event.event_name = event_name;
        draft_event.event.event_desc = description;
        draft_event.event.thumbnail_link = thumbnail_link;
        draft_event.event.message_id = String::new();
        draft_event.event.event_time = date.naive_utc();

        draft_event.creator_id = msg.author.id.0;
    }

    {
        let data = ctx.data.read();
        msg.reply(&ctx, format!("Draft message, use the `confirm_event` command to confirm it."))?;
        send_event_msg(ctx, msg.channel_id.0, &data.get::<DraftEvent>().unwrap().event)?;
    }

    Ok(())
}
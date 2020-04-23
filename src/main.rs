#[macro_use]
extern crate diesel;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate diesel_migrations;
extern crate serde;

use serenity::client::Client;
use serenity::model::channel::Message;
use serenity::prelude::{EventHandler, Context};
use serenity::utils::{content_safe, ContentSafeOptions, Colour};
use serenity::model::gateway::Ready;
use serenity::framework::standard::StandardFramework;
use serenity::framework::standard::CommandResult;
use serenity::framework::standard::macros::{command, group,};
use serenity::framework::standard::Args;
use clap::{Arg, App};
use chrono::{DateTime, Utc};
use std::process::exit;

mod hypebot_config;
mod database;

use database::*;
use hypebot_config::HypeBotConfig;

/// Event commands group
#[group]
#[commands(create_event)]
struct EventCommands;

/// Handler for Discord events
struct Handler;

impl EventHandler for Handler {
    /// On bot ready
    fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
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

#[command]
/// Creates an event and announce it
fn create_event (ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Open config
    let data = ctx.data.read();
    let config = data.get::<HypeBotConfig>().unwrap();

    // Parse args
    let event_name = args.single::<String>()?.replace("\"", "");
    let date_string = args.single::<String>()?.replace("\"", "");
    let description = args.single::<String>()?.replace("\"", "");
    let thumbnail_link = match args.single::<String>() {
        Ok(link) => link,
        Err(_) => config.default_thumbnail_link.clone(),
    };

    // Parse date
    let date = DateTime::parse_from_str(date_string.as_str(),
                                        "%H:%M %z %Y-%m-%d").unwrap();
    let date = DateTime::<Utc>::from(date);

    // Clean channel, role, and everyone pings
    let settings = ContentSafeOptions::default()
        .clean_channel(true)
        .clean_role(true)
        .clean_everyone(true)
        .display_as_member_from(msg.guild_id.unwrap());

    let description = content_safe(&ctx.cache, description, &settings);

    let channel = ctx.http.get_channel(config.event_channel)?;

    // Send message
    let msg = channel.id().send_message(&ctx, |m| {
        m.embed(|e| {
            e.title(event_name.as_str())
                .color(Colour::PURPLE)
                .description(format!("**{}**\n{}", date.format("%A, %B %d @ %I:%M %P %t %Z"), description))
                .thumbnail(&thumbnail_link)
                .footer(|f| { f.text("Local Event Time") })
                .timestamp(date.to_rfc3339())
        })
    })?;

    // Add reacts
    msg.react(&ctx, "\u{2705}")?;
    msg.react(&ctx, "\u{274C}")?;

    // Insert event into the database
    insert_event(config.db_url.clone(), event_name.as_str(),
                 description.as_str(), &date.naive_utc(),
                 format!("{}", msg.id.0).as_str(),
                 thumbnail_link.as_str());


    Ok(())
}
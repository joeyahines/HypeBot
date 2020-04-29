use crate::database::get_event_by_msg_id;
use crate::database::models::NewEvent;
use crate::hypebot_config::HypeBotConfig;
use crate::{DraftEvent, INTERESTED_EMOJI, UNINTERESTED_EMOJI};
use chrono::{DateTime, NaiveDateTime, Utc};
use serenity::framework::standard::{CommandError, CommandResult};
use serenity::http::Http;
use serenity::model::prelude::{ChannelId, Message, Reaction, User};
use serenity::prelude::{Context, RwLock, ShareMap};
use serenity::utils::Colour;
use serenity::Result;
use std::sync::Arc;

pub mod events;

/// Send a message to a reaction user
///
/// Message will be sent in the format
/// ```
/// "{msg_text} **event_name**"
/// ```
pub fn send_message_to_reaction_users(ctx: &Context, reaction: &Reaction, msg_text: &str) {
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
pub fn send_dm_message(http: &Http, user: User, message: &String) {
    if let Ok(dm_channel) = user.create_dm_channel(&http) {
        dm_channel.send_message(&http, |m| m.content(message)).ok();
    }
}

/// Sends the event message to the event channel
pub fn send_event_msg(
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
                    "**{}**\n{}\n\nReact with {} below to receive event reminders!",
                    native_time.format("%A, %B %d @ %I:%M %P %t %Z"),
                    event.event_desc,
                    INTERESTED_EMOJI
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
pub fn update_draft_event(
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
pub fn send_draft_event(ctx: &Context, channel: ChannelId) -> CommandResult {
    let data = ctx.data.read();
    let config = data
        .get::<HypeBotConfig>()
        .ok_or(CommandError("Config not found!".to_string()))?;
    let draft_event = data
        .get::<DraftEvent>()
        .ok_or(CommandError("Draft event not found!".to_string()))?;

    channel.send_message(&ctx, |m| {
        m.content(format!(
            "Draft message, use the `confirm` command to post it."
        ))
    })?;
    send_event_msg(&ctx.http, config, channel.0, &draft_event.event, false)?;
    Ok(())
}

/// Gets the config from context data
pub fn get_config(
    data: &Arc<RwLock<ShareMap>>,
) -> std::result::Result<HypeBotConfig, CommandError> {
    let data_read = data.read();
    let config = data_read
        .get::<HypeBotConfig>()
        .ok_or(CommandError("Unable to get config".to_string()))?;

    Ok(config.clone())
}

/// Logs command errors to the logger
pub fn log_error(
    _ctx: &mut Context,
    _msg: &Message,
    command_name: &str,
    result: std::result::Result<(), CommandError>,
) {
    match result {
        Ok(()) => (),
        Err(why) => error!("Command '{}' returned error {:?}", command_name, why),
    };
}

/// Checks if the user has permission to use this bot
pub fn permission_check(ctx: &mut Context, msg: &Message, _command_name: &str) -> bool {
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

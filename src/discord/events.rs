use super::{get_config, send_event_msg};
use crate::database::{get_event_by_name, insert_event, remove_event};
use crate::discord::{send_dm_message, send_draft_event, update_draft_event};
use crate::{DraftEvent, INTERESTED_EMOJI};
use chrono::offset::TimeZone;
use chrono::{Datelike, NaiveDateTime, Timelike, Utc};
use chrono_tz::Tz;
use serenity::framework::standard::{macros::command, Args, CommandError, CommandResult};
use serenity::model::prelude::{Message, User};
use serenity::prelude::Context;
use serenity::utils::{content_safe, ContentSafeOptions};

#[command]
/// Posts a previewed event
///
/// **Note**
/// You can only post events you have created. Only one preview event can exist at a time.
fn confirm(ctx: &mut Context, msg: &Message, _args: Args) -> CommandResult {
    let config = get_config(&ctx.data)?;
    let data = ctx.data.read();

    // Get draft event
    if let Some(draft_event) = data.get::<DraftEvent>() {
        let mut new_event = draft_event.event.clone();
        // Check to to see if message author is the owner of the pending event
        if draft_event.creator_id == msg.author.id.0 {
            // Send event message
            let event_msg =
                send_event_msg(&ctx.http, &config, config.event_channel, &new_event, true)?;

            msg.reply(&ctx, "Event posted!")?;

            new_event.message_id = event_msg.id.0.to_string();

            insert_event(config.db_url.clone(), &new_event)?;
        } else {
            msg.reply(&ctx, format!("You do not have a pending event!"))?;
        }
    } else {
        msg.reply(&ctx, format!("There are no pending events!!"))?;
    }

    Ok(())
}

#[command]
/// Creates an event and previews the announcement
///
/// `~create "event name" "04:20pm 2069-04-20" "event description" <http://optional.thumbnail.link>`
///
/// **Time format**
/// The time format is HH:MMam YYYY-MM-DD
///
/// **Thumbnail Link**
/// The thumbnail link is optional, if one is not provided, a default image is shown
fn create(ctx: &mut Context, msg: &Message, mut args: Args) -> CommandResult {
    // Get config
    let config = get_config(&ctx.data)?;
    let guild_id = msg
        .guild_id
        .ok_or(CommandError("Unable to get guild ID".to_string()))?;

    // Parse args
    let event_name = match args.single::<String>() {
        Ok(event_name) => event_name.replace("\"", ""),
        Err(_) => {
            msg.reply(&ctx, "No event name provided.".to_string())?;
            return Ok(());
        }
    };
    let date_string = match args.single::<String>() {
        Ok(date_string) => date_string.replace("\"", ""),
        Err(_) => {
            msg.reply(&ctx, "No date provided.".to_string())?;
            return Ok(());
        }
    };
    let description = match args.single::<String>() {
        Ok(desc) => desc.replace("\"", ""),
        Err(_) => {
            msg.reply(&ctx, "No description provided.".to_string())?;
            return Ok(());
        }
    };
    let thumbnail_link = match args.single::<String>() {
        Ok(link) => link.replace("<", "").replace(">", ""),
        Err(_) => config.default_thumbnail_link.clone(),
    };

    // Parse date
    let tz: Tz = config.event_timezone;
    let input_date = match NaiveDateTime::parse_from_str(date_string.as_str(), "%I:%M%P %Y-%m-%d") {
        Ok(date) => date,
        Err(_) => {
            msg.reply(
                &ctx,
                "Invalid date format. Format is HH:MMam YYYY-MM-DD".to_string(),
            )?;
            return Ok(());
        }
    };

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
/// Cancels an already scheduled event
///
/// `~create_event "event name"`
fn cancel(ctx: &mut Context, _msg: &Message, mut args: Args) -> CommandResult {
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

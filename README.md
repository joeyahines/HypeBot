# HypeBot
A Discord bot for managing events. Uses [Serenity](https://github.com/serenity-rs/serenity) for the bot framework
and [Diesel](http://diesel.rs/) as an ORM.

## Usage
Creating an event can be done using the `create` command.
```
~create "Test Event" "5:35PM 2020-05-17" "A very cool test event!" "Cool Place"
```

This creates a draft event that the user can then review:

![create event example](https://i.imgur.com/9jTko9W.png)

A user can then use the `confirm` command to create the event and publish it:

![announcement](https://i.imgur.com/AeTE1v2.png)

Users who react with ✅ will then be sent reminders about the event as private message.

## Running
`./hype_bot config.toml`

## Config
```toml
# Database URL
db_url = "mysql://[user]:[password]@localhost/hypebot_db"
# Default image to show on the thumbnail
default_thumbnail_link = "https://i.imgur.com/wPdnvoE.png"
# Discord bot key
discord_key = ""
# Bot command prefix
prefix = "~"
# Channel ID to post to
event_channel = 0
# List of roles that can use the bot
event_roles = [0]
# Timezone to display events, supported timezones can be found at https://docs.rs/chrono-tz/0.5.1/chrono_tz/#modules
event_timezone = "America/New_York"
# Path to logger
log_path = "hype_bot.log"
```
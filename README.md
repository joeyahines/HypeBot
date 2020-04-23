# HypeBot
A Discord bot for managing events. Uses [Serenity](https://github.com/serenity-rs/serenity) for the bot framework
and [Disel](http://diesel.rs/) as an ORM.

## Running
`./hype_bot -c config.toml`

##Config
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
# Permissions to use the bot
event_roles = 0
```
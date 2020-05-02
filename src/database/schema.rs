table! {
    events (id) {
        id -> Integer,
        event_name -> Varchar,
        event_desc -> Varchar,
        event_loc -> Varchar,
        organizer -> Varchar,
        event_time -> Datetime,
        message_id -> Varchar,
        thumbnail_link -> Varchar,
        reminder_sent -> Integer,
    }
}

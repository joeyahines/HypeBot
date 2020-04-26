use super::schema::events;
use chrono::{NaiveDateTime};

#[derive(Queryable)]
pub struct Event {
    pub id: i32,
    pub event_name: String,
    pub event_desc: String,
    pub event_time: NaiveDateTime,
    pub message_id: String,
    pub thumbnail_link: String,
    pub reminder_sent: i32,
}

#[derive(Insertable)]
#[table_name="events"]
pub struct NewEvent {
    pub event_name: String,
    pub event_desc: String,
    pub event_time: NaiveDateTime,
    pub message_id: String,
    pub thumbnail_link: String,
    pub reminder_sent: i32,
}

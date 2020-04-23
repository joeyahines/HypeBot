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
}

#[derive(Insertable)]
#[table_name="events"]
pub struct NewEvent<'a> {
    pub event_name: &'a str,
    pub event_desc: &'a str,
    pub event_time: &'a NaiveDateTime,
    pub message_id: &'a str,
    pub thumbnail_link: &'a str,
}

use super::schema::events;
use chrono::NaiveDateTime;

#[derive(Queryable)]
pub struct Event {
    /// Event ID
    pub id: i32,
    /// Event name
    pub event_name: String,
    /// Event long description
    pub event_desc: String,
    /// Event datetime
    pub event_time: NaiveDateTime,
    /// Event discord message id
    pub message_id: String,
    /// Event message thumbnail link
    pub thumbnail_link: String,
    /// Reminder sent tracker
    pub reminder_sent: i32,
}

#[derive(Insertable, Clone)]
#[table_name = "events"]
pub struct NewEvent {
    /// Event name
    pub event_name: String,
    /// Event long description
    pub event_desc: String,
    /// Event datetime
    pub event_time: NaiveDateTime,
    /// Event discord message id
    pub message_id: String,
    /// Event message thumbnail link
    pub thumbnail_link: String,
    /// Reminder sent tracker
    pub reminder_sent: i32,
}

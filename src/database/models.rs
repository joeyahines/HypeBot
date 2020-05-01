use super::schema::events;
use chrono::NaiveDateTime;

#[derive(Queryable, Clone, Debug)]
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

impl Into<NewEvent> for Event {
    fn into(self) -> NewEvent {
        NewEvent {
            event_name: self.event_name.clone(),
            event_desc: self.event_desc.clone(),
            event_time: self.event_time.clone(),
            message_id: self.message_id.clone(),
            thumbnail_link: self.message_id.clone(),
            reminder_sent: self.reminder_sent,
        }
    }
}

#[derive(Insertable, Clone, Debug)]
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

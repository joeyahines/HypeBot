pub mod schema;
pub mod models;

use diesel::prelude::*;
use chrono::NaiveDateTime;
use models::{Event, NewEvent};

pub fn establish_connection(database_url: String) -> MysqlConnection {
    MysqlConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub fn insert_event(databse_url: String, event_name: &str, event_desc: &str,
                        event_time: &NaiveDateTime, message_id: &str, thumbnail_link: &str) -> Event {
    use schema::events::dsl::{id, events};

    let new_event = NewEvent {
       event_name,
        event_desc,
        event_time,
        message_id,
        thumbnail_link,
    };

    let connection = establish_connection(databse_url);

    diesel::insert_into(events)
        .values(&new_event)
        .execute(&connection)
        .expect("Error saving event");

    events.order(id).first(&connection).unwrap()
}

#[warn(dead_code)]
pub mod models;
pub mod schema;

use diesel::prelude::*;
use diesel::result::Error;
use diesel::update;
use models::{Event, NewEvent};
use std::vec::Vec;

/// Establish a connection to the database
pub fn establish_connection(database_url: String) -> MysqlConnection {
    MysqlConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}
/// Insert an event into the database
pub fn insert_event(databse_url: String, new_event: &NewEvent) -> Event {
    use schema::events::dsl::{events, id};

    let connection = establish_connection(databse_url);

    diesel::insert_into(events)
        .values(new_event)
        .execute(&connection)
        .expect("Error saving event");

    events.order(id).first(&connection).unwrap()
}

/// Get an event by name
pub fn get_event_by_name(database_url: String, name: String) -> Result<Event, Error> {
    use schema::events::dsl::{event_name, events};

    let connection = establish_connection(database_url);

    events
        .filter(event_name.eq(&name))
        .get_result::<Event>(&connection)
}

/// Get event by its message id
pub fn get_event_by_msg_id(database_url: String, msg_id: String) -> Result<Event, Error> {
    use schema::events::dsl::{events, message_id};

    let connection = establish_connection(database_url);

    events
        .filter(message_id.eq(&msg_id))
        .get_result::<Event>(&connection)
}

/// Get all events
pub fn get_all_events(database_url: String) -> Result<Vec<Event>, Error> {
    use schema::events::dsl::{event_time, events};

    let connection = establish_connection(database_url);

    events.order(event_time).load(&connection)
}

/// Set the reminder state of an event
pub fn set_reminder(database_url: String, event_id: i32, state: i32) -> Result<usize, Error> {
    use schema::events::dsl::{events, id, reminder_sent};
    let connection = establish_connection(database_url);

    let target = events.filter(id.eq(event_id));
    update(target)
        .set(reminder_sent.eq(state))
        .execute(&connection)
}

pub mod schema;
pub mod models;

use diesel::prelude::*;
use models::{Event, NewEvent};
use diesel::result::Error;
use diesel::update;
use std::vec::Vec;

pub fn establish_connection(database_url: String) -> MysqlConnection {
    MysqlConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

pub fn insert_event(databse_url: String, new_event: &NewEvent) -> Event {
    use schema::events::dsl::{id, events};

    let connection = establish_connection(databse_url);

    diesel::insert_into(events)
        .values(new_event)
        .execute(&connection)
        .expect("Error saving event");

    events.order(id).first(&connection).unwrap()
}

pub fn get_event_by_name(database_url: String, name: String) -> Result<Event, Error> {
    use schema::events::dsl::{event_name, events};

    let connection = establish_connection(database_url);

    events.filter(event_name.eq(&name)).get_result::<Event>(&connection)
}

pub fn get_event_by_msg_id(database_url: String, msg_id: String) -> Result<Event, Error> {
    use schema::events::dsl::{message_id, events};

    let connection = establish_connection(database_url);

    events.filter(message_id.eq(&msg_id)).get_result::<Event>(&connection)
}

pub fn get_all_events(database_url: String) -> Result<Vec<Event>, Error> {
    use schema::events::dsl::{event_time, events};

    let connection = establish_connection(database_url);

    events.order(event_time).load(&connection)
}

pub fn set_reminder(database_url: String, event_id: i32) -> Result<usize, Error> {
    use schema::events::dsl::{events, id, reminder_sent};
    let connection = establish_connection(database_url);

    let target = events.filter(id.eq(event_id));
    update(target).set(reminder_sent.eq(1)).execute(&connection)
}
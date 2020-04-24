pub mod schema;
pub mod models;

use diesel::prelude::*;
use models::{Event, NewEvent};

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

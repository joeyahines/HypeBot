-- Your SQL goes here
CREATE TABLE events (
  id INTEGER AUTO_INCREMENT PRIMARY KEY,
  event_name VARCHAR(255) NOT NULL,
  event_desc VARCHAR(255) NOT NULL,
  event_time DATETIME NOT NULL,
  message_id VARCHAR(255) NOT NULL,
  thumbnail_link VARCHAR(255) NOT NULL
)
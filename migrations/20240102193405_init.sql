CREATE TABLE organizations
(
	id   TEXT NOT NULL PRIMARY KEY,
	name TEXT NOT NULL
);

CREATE TABLE events
(
	id           TEXT NOT NULL PRIMARY KEY,
	organization TEXT NOT NULL,
	name         TEXT NOT NULL,
	FOREIGN KEY (organization) REFERENCES organizations (id) ON DELETE CASCADE
);

CREATE TABLE registration_schema_items
(
	id                         TEXT                                                                                     NOT NULL PRIMARY KEY,
	event                      TEXT                                                                                     NOT NULL,
	idx                        INTEGER                                                                                  NOT NULL,
	name                       TEXT                                                                                     NOT NULL,
	item_type                  TEXT CHECK( item_type IN ("TextType", "CheckboxType", "SelectType", "MultiSelectType") ) NOT NULL,
	text_type_default          TEXT,
	text_type_display          TEXT CHECK( text_type_display IN ("SMALL", "LARGE") ),
	checkbox_type_default      INTEGER CHECK( checkbox_type_default IN (TRUE, FALSE) ),
	select_type_default        INTEGER,
	select_type_display        TEXT CHECK( select_type_display IN ("RADIO", "DROPDOWN") ),
	multi_select_type_defaults TEXT,
	multi_select_type_display  TEXT CHECK( multi_select_type_display IN ("CHECKBOXES", "MULTISELECT_BOX") ),
	FOREIGN KEY (event) REFERENCES events (id) ON DELETE CASCADE
);

CREATE TABLE registration_schema_select_options
(
	id          TEXT    NOT NULL PRIMARY KEY,
	schema_item TEXT    NOT NULL,
	idx         INTEGER NOT NULL,
	name        TEXT    NOT NULL,
	product_id  TEXT    NOT NULL,
	FOREIGN KEY (schema_item) REFERENCES registration_schema_items (id) ON DELETE CASCADE
);

CREATE TABLE registrations
(
	id    TEXT NOT NULL PRIMARY KEY,
	event TEXT NOT NULL,
	FOREIGN KEY (event) REFERENCES events (id) ON DELETE CASCADE
);

CREATE TABLE registration_items
(
	id            TEXT NOT NULL PRIMARY KEY,
	registration  TEXT NOT NULL,
	schema_item   TEXT NOT NULL,
	value         TEXT NOT NULL,
	FOREIGN KEY (registration) REFERENCES registrations (id) ON DELETE CASCADE,
	FOREIGN KEY (schema_item) REFERENCES registration_schema_items (id) ON DELETE CASCADE

);

CREATE TABLE users
(
	id           TEXT NOT NULL PRIMARY KEY,
	username     TEXT NOT NULL UNIQUE,
	password     TEXT,
	email        TEXT
);

CREATE TABLE keys
(
	id         TEXT NOT NULL PRIMARY KEY,
	eddsa_key  BLOB NOT NULL,
	created_at INT NOT NULL
);

CREATE TABLE permissions
(
	id           TEXT                                                                                                                               NOT NULL PRIMARY KEY,
	user         TEXT                                                                                                                               NOT NULL,
	role         TEXT CHECK( role IN ("SERVER_ADMIN", "ORGANIZATION_ADMIN", "ORGANIZATION_VIEWER", "EVENT_ADMIN", "EVENT_EDITOR", "EVENT_VIEWER") ) NOT NULL,
	organization TEXT,
	event        TEXT,
	FOREIGN KEY (user) REFERENCES users (id) ON DELETE CASCADE
	FOREIGN KEY (organization) REFERENCES organizations (id) ON DELETE CASCADE
	FOREIGN KEY (event) REFERENCES events (id) ON DELETE CASCADE
);

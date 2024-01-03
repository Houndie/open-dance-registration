CREATE TABLE events
(
	id   BLOB PRIMARY_KEY NOT NULL,
	name TEXT             NOT NULL
);

CREATE TABLE registration_schema_items
(
	id                         BLOB PRIMARY_KEY                                                                         NOT NULL,
	event                      BLOB                                                                                     NOT NULL,
	idx                        INTEGER                                                                                  NOT NULL,
	name                       TEXT                                                                                     NOT NULL,
	item_type                  TEXT CHECK( item_type IN ("TextType", "CheckboxType", "SelectType", "MultiSelectType") ) NOT NULL,
	text_type_default          TEXT,
	text_type_display          TEXT CHECK( text_type_display IN ("SMALL", "LARGE") ),
	checkbox_type_default      INTEGER CHECK( checkbox_type_default IN (TRUE, FALSE) ),
	select_type_default        INTEGER,
	select_type_display        TEXT CHECK( select_type_display IN ("RADIO", "DROPDOWN") ),
	select_type_options        BLOB,
	multi_select_type_defaults BLOB,
	multi_select_type_display  TEXT CHECK( multi_select_type_display IN ("CHECKBOXES", "MULTISELECT_BOX") ),
	multi_select_type_options  BLOB
);

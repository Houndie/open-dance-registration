use crate::{
    hooks::{
        handle_error::use_handle_error,
        toasts::{use_toasts, ToastManager},
    },
    proto::{
        self, event_query, multi_select_type, organization_query,
        registration_schema_item_type::Type as ItemType, registration_schema_query, select_type,
        string_query, text_type, CheckboxType, ClaimsRequest, EventQuery, MultiSelectType,
        Organization, OrganizationQuery, QueryEventsRequest, QueryOrganizationsRequest,
        QueryRegistrationSchemasRequest, RegistrationSchema, RegistrationSchemaItem,
        RegistrationSchemaItemType, RegistrationSchemaQuery, SelectOption, SelectType, StringQuery,
        TextType, UpsertRegistrationSchemasRequest,
    },
    server_functions::{
        authentication::claims, event::query as query_events,
        organization::query as query_organizations,
        registration_schema::query as query_registration_schemas,
        registration_schema::upsert as upsert_registration_schema, ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{
            form::{
                Button, ButtonFlavor, CheckInput, CheckStyle, Field, SelectInput, TextInput,
                TextInputType,
            },
            modal::Modal,
            page::Page as GenericPage,
            table::Table,
        },
        pages::event::{Menu, MenuItem},
    },
};
use dioxus::prelude::*;
use futures::join;
use std::{
    collections::{BTreeSet, HashMap},
    rc::Rc,
};
use strum::{EnumIter, IntoEnumIterator};
use uuid::Uuid;

fn default_registration_schema_item() -> RegistrationSchemaItem {
    RegistrationSchemaItem {
        id: String::default(),
        name: String::default(),
        r#type: Some(RegistrationSchemaItemType {
            r#type: Some(ItemType::Text(TextType {
                default: String::default(),
                display: text_type::Display::Small as i32,
            })),
        }),
    }
}

#[derive(Default, Clone)]
struct Schema {
    items: Vec<(Uuid, RegistrationSchemaItem)>,
    event_id: String,
}

impl From<Schema> for RegistrationSchema {
    fn from(schema: Schema) -> Self {
        RegistrationSchema {
            event_id: schema.event_id,
            items: schema.items.into_iter().map(|(_, i)| i).collect(),
        }
    }
}

#[derive(Clone)]
struct DragData {
    dragged: usize,
    new_location: usize,
    line_location: Option<LineLocation>,
}

#[derive(Clone)]
struct LineLocation {
    top: f64,
    left: f64,
    width: f64,
}

#[component]
pub fn Page(id: ReadOnlySignal<String>) -> Element {
    let results = use_server_future(move || async move {
        let claims_future = claims(ClaimsRequest {});

        let events_future = query_events(QueryEventsRequest {
            query: Some(EventQuery {
                query: Some(event_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(id())),
                })),
            }),
        });

        let schemas_future = query_registration_schemas(QueryRegistrationSchemasRequest {
            query: Some(RegistrationSchemaQuery {
                query: Some(registration_schema_query::Query::EventId(StringQuery {
                    operator: Some(string_query::Operator::Equals(id())),
                })),
            }),
        });

        let _ = claims_future.await.map_err(Error::from_server_fn_error)?;

        let mut events_response = events_future.await.map_err(Error::from_server_fn_error)?;
        let event = events_response.events.pop().ok_or(Error::NotFound)?;

        let organizations_future = query_organizations(QueryOrganizationsRequest {
            query: Some(OrganizationQuery {
                query: Some(organization_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(
                        event.organization_id.clone(),
                    )),
                })),
            }),
        });

        let mut schemas_response = schemas_future.await.map_err(Error::from_server_fn_error)?;
        let schema = schemas_response
            .registration_schemas
            .pop()
            .unwrap_or_else(|| RegistrationSchema {
                event_id: id(),
                ..Default::default()
            });

        let mut organizations_response = organizations_future
            .await
            .map_err(Error::from_server_fn_error)?;
        let organization = organizations_response
            .organizations
            .pop()
            .ok_or(Error::Misc("organization not found".to_owned()))?;

        Ok((
            ProtoWrapper(event),
            ProtoWrapper(schema),
            ProtoWrapper(organization),
        ))
    })?;

    use_handle_error(
        results.suspend()?,
        |(ProtoWrapper(event), ProtoWrapper(schema), ProtoWrapper(organization))| {
            let grabbing_cursor = use_signal(|| false);
            let cursor = use_memo(move || {
                if *grabbing_cursor.read() {
                    "cursor: grabbing; cursor: -moz-grabbing; cursor: -webkit-grabbing;"
                } else {
                    "cursor: auto"
                }
                .to_owned()
            });

            rsx! {
                GenericPage {
                    title: "Modify Registration Schema".to_owned(),
                    style: Into::<ReadOnlySignal<String>>::into(cursor),
                    breadcrumb: vec![
                        ("Home".to_owned(), Some(Routes::LandingPage)),
                        (organization.name.clone(), Some(Routes::OrganizationPage { org_id: organization.id.clone() })),
                        (event.name.clone(), Some(Routes::EventPage{ id: event.id.clone() })),
                        ("Registration Schema".to_owned(), None),
                    ],
                    menu: rsx!{
                        Menu {
                            event_name: event.name.clone(),
                            event_id: event.id.clone(),
                            highlight: MenuItem::RegistrationSchema,
                        }
                    },
                    PageBody{
                        org: organization,
                        event: event,
                        registration_schema_items: schema.items,
                        grabbing_cursor: grabbing_cursor,
                    }
                }
            }
        },
    )
}

#[component]
fn PageBody(
    org: ReadOnlySignal<Organization>,
    event: ReadOnlySignal<proto::Event>,
    registration_schema_items: ReadOnlySignal<Vec<RegistrationSchemaItem>>,
    grabbing_cursor: Signal<bool>,
) -> Element {
    let mut toaster = use_toasts();

    let mut show_schema_item_modal = use_signal(|| None);
    let mut show_delete_item_modal = use_signal(|| None);

    let mut table_row_refs = use_signal(HashMap::default);
    let drag_data = use_signal(|| None);

    let drag_line = drag_data.read().as_ref().and_then(|d: &DragData| d.line_location.as_ref()).map(|absolute_location| {
        rsx!{
            hr {
                style: "position: fixed; top: {absolute_location.top}px; left: {absolute_location.left}px; height: 5px; width: {absolute_location.width}px; margin-top: 0; margin-bottom: 0; background-color: black;",
            }
        }
    });

    let mut schema: Signal<Schema> = use_signal(move || {
        registration_schema_items.with(|schema| Schema {
            items: schema
                .iter()
                .map(|item| (Uuid::new_v4(), item.clone()))
                .collect(),
            event_id: event().id.clone(),
        })
    });

    let schema_item_modal = {
        show_schema_item_modal.read().as_ref().map(move |(key, item): &(Uuid, RegistrationSchemaItem)| {
            let key = *key;
            let item = item.clone();
            rsx!{
                SchemaItemModal{
                    initial: item,
                    grabbing_cursor: grabbing_cursor,
                    onsubmit: move |item: RegistrationSchemaItem| {
                        let mut send_schema = schema.read().clone();
                        let is_new = item.id.is_empty();
                        if is_new {
                            send_schema.items.push((key, item.clone()));
                        } else {
                            let idx = send_schema.items.iter().position(|(_, i)| i.id == item.id);
                            match idx {
                                Some(idx) => send_schema.items[idx] = (key, item.clone()),
                                None => {
                                    toaster.write().new_error("Item not found".to_owned());
                                    return;
                                },
                            }
                        }

                        spawn(async move {
                            let registration_schema = RegistrationSchema {
                                event_id: send_schema.event_id.clone(),
                                items: send_schema.items.iter().cloned().map(|(_, i)| i).collect(),
                            };

                            let rsp = upsert_registration_schema(UpsertRegistrationSchemasRequest{
                                registration_schemas: vec![registration_schema],
                            }).await;

                            let mut rsp = match rsp {
                                Ok(rsp) => rsp,
                                Err(e) => {
                                    toaster.write().new_error(e.to_string());
                                    return
                                }
                            };

                            if is_new {
                                send_schema.items.last_mut().unwrap().1.id = rsp.registration_schemas[0].items.pop().unwrap().id;
                            }

                            *schema.write() = send_schema;
                            show_schema_item_modal.set(None);
                        });
                    },
                    onclose: move |_| show_schema_item_modal.set(None),
                }
            }
        })
    };

    let delete_item_modal = {
        show_delete_item_modal.read().as_ref().map(move |idx: &usize| {
            let idx = *idx;
            rsx!{
                DeleteItemModal{
                    onsubmit: move |_| {
                        let registration_schema = RegistrationSchema {
                            event_id: schema.read().event_id.clone(),
                            items: schema.read().items.iter().enumerate().filter(|(i, _)| *i != idx).map(|(_, (_, i))| i).cloned().collect(),
                        };

                        spawn(async move {
                            let rsp = upsert_registration_schema(UpsertRegistrationSchemasRequest{
                                registration_schemas: vec![registration_schema],
                            }).await;

                            match rsp {
                                Ok(_) => {},
                                Err(e) => {
                                    toaster.write().new_error(e.to_string());
                                }
                            }
                        });

                        schema.write().items.remove(idx);
                        table_row_refs.write().remove(&schema.read().items[idx].0);
                        show_delete_item_modal.set(None);
                    },
                    onclose: move |_| show_delete_item_modal.set(None),
                }
            }
        })
    };

    rsx! {
        Table {
            is_striped: true,
            is_fullwidth: true,
            thead {
                tr {
                    th{
                        style: "width: 1px",
                    }
                    th {
                        class: "col-auto",
                        "Item"
                    }
                    th{
                        style: "width: 1px",
                    }
                    th{
                        style: "width: 1px",
                    }
                }
            }
            tbody {
                { schema.read().items.iter().enumerate().map(move |(idx, (key, i))| {
                    let item = i.clone();
                    let key = *key;

                    rsx!{
                        tr {
                            key: "{key}",
                            onmounted: move |d| { table_row_refs.write().insert(key, d.data); },
                            ondragover: move |dragover| {
                                spawn(async move {
                                    ondragover(dragover, drag_data, table_row_refs.clone(), toaster, idx, key, None, None).await;
                                });
                            },
                            td {
                                {
                                    rsx! {
                                        OptionGrab {
                                            drag_data: drag_data,
                                            grabbing_cursor: grabbing_cursor,
                                            idx: idx,
                                            ondragend: move |data: DragData| {
                                                spawn(async move {
                                                    let mut schema_copy = schema.read().clone();
                                                    if data.dragged < data.new_location {
                                                        schema_copy.items[data.dragged..=data.new_location].rotate_left(1);
                                                    } else {
                                                        schema_copy.items[data.new_location..=data.dragged].rotate_right(1);
                                                    }

                                                    *schema.write() = schema_copy.clone();


                                                    let res = upsert_registration_schema(UpsertRegistrationSchemasRequest{
                                                        registration_schemas: vec![schema_copy.into()],
                                                    }).await;

                                                    if let Err(e) = res {
                                                        toaster.write().new_error(e.to_string());
                                                    }
                                                });
                                            },
                                        }
                                    }
                                }
                            }
                            td{
                                class: "col-auto",
                                "{item.name}"
                            }
                            td{
                                style: "width: 1px",
                                Button {
                                    flavor: ButtonFlavor::Info,
                                    onclick: move |_| show_schema_item_modal.set(Some((key.clone(), item.clone()))),
                                    "Edit"
                                }
                            }
                            td{
                                Button {
                                    flavor: ButtonFlavor::Danger,
                                    onclick: move |_| { show_delete_item_modal.set(Some(idx));},
                                    "Delete"
                                }
                            }
                        }
                    }
                }) }
            }
        }
        Button {
            flavor: ButtonFlavor::Info,
            onclick: move |_| show_schema_item_modal.set(Some((Uuid::new_v4(), default_registration_schema_item()))),
            "Add Field"
        }
        { drag_line }
        { schema_item_modal }
        { delete_item_modal }
    }
}

#[derive(EnumIter, PartialEq, strum::Display)]
enum ItemFieldsType {
    Text,
    Checkbox,
    Select,
    MultiSelect,
}

#[derive(EnumIter, PartialEq, strum::Display)]
enum TextDisplayType {
    Small,
    Large,
}

#[derive(EnumIter, PartialEq, strum::Display)]
enum SelectDisplayType {
    Radio,
    Dropdown,
}

#[derive(EnumIter, PartialEq, strum::Display)]
enum MultiSelectDisplayType {
    Checkboxes,
    MultiselectBox,
}

#[derive(Default, Clone)]
struct FieldsSelect {
    display: usize,
}

#[derive(Default, Clone)]
struct FieldsMultiSelect {
    display: usize,
}

#[derive(Default, Clone)]
struct FieldsText {
    default: String,
    display: usize,
}

#[derive(Clone, Debug)]
struct FieldSelectOption {
    key: Uuid,
    option: SelectOption,
}

impl FieldSelectOption {
    fn default() -> Self {
        FieldSelectOption {
            key: Uuid::new_v4(),
            option: SelectOption::default(),
        }
    }
}

#[derive(Clone, Default)]
struct ItemFields {
    id: String,
    name: String,
    name_touched: bool,
    typ: usize,
    text_type: FieldsText,
    checkbox_type: CheckboxType,
    select_type: FieldsSelect,
    defaults: BTreeSet<usize>,
    multi_select_type: FieldsMultiSelect,
    options: Vec<FieldSelectOption>,
    validation_error: Option<String>,
}

fn enum_selects<Enum: IntoEnumIterator + std::fmt::Display>() -> Vec<(Enum, String)> {
    Enum::iter()
        .map(|e| {
            let estr = format!("{}", e);
            (e, estr)
        })
        .collect::<Vec<_>>()
}

#[component]
fn SchemaItemModal(
    initial: ReadOnlySignal<RegistrationSchemaItem>,
    onsubmit: EventHandler<RegistrationSchemaItem>,
    onclose: EventHandler<()>,
    grabbing_cursor: Signal<bool>,
) -> Element {
    let mut toaster = use_toasts();
    let type_selects = use_memo(enum_selects::<ItemFieldsType>);
    let text_display_selects = use_memo(enum_selects::<TextDisplayType>);
    let select_display_selects = use_memo(enum_selects::<SelectDisplayType>);
    let multi_select_display_selects = use_memo(enum_selects::<MultiSelectDisplayType>);
    let drag_data = use_signal(|| None);
    let mut field_refs = use_signal(HashMap::default);
    let success_text = use_memo(move || {
        if initial().id.is_empty() {
            "Create"
        } else {
            "Update"
        }
    });

    let mut fields = use_signal(|| {
        let item = initial().clone();

        let (typ, text_type, checkbox_type, defaults, select_type, multi_select_type, options) =
            match item.r#type.unwrap().r#type.unwrap() {
                ItemType::Text(text) => (
                    0,
                    FieldsText {
                        default: text.default,
                        display: text.display as usize,
                    },
                    CheckboxType::default(),
                    BTreeSet::default(),
                    FieldsSelect::default(),
                    FieldsMultiSelect::default(),
                    Vec::default(),
                ),
                ItemType::Checkbox(checkbox) => (
                    1,
                    FieldsText::default(),
                    checkbox,
                    BTreeSet::default(),
                    FieldsSelect::default(),
                    FieldsMultiSelect::default(),
                    Vec::default(),
                ),
                ItemType::Select(select) => {
                    let options = select
                        .options
                        .into_iter()
                        .map(|option| {
                            let key = Uuid::new_v4();
                            FieldSelectOption { key, option }
                        })
                        .collect();

                    (
                        2,
                        FieldsText::default(),
                        CheckboxType::default(),
                        BTreeSet::from([select.default as usize]),
                        FieldsSelect {
                            display: select.display as usize,
                        },
                        FieldsMultiSelect::default(),
                        options,
                    )
                }
                ItemType::MultiSelect(multiselect) => {
                    let options = multiselect
                        .options
                        .into_iter()
                        .map(|option| {
                            let key = Uuid::new_v4();
                            FieldSelectOption { key, option }
                        })
                        .collect();
                    (
                        3,
                        FieldsText::default(),
                        CheckboxType::default(),
                        multiselect
                            .defaults
                            .into_iter()
                            .map(|d| d as usize)
                            .collect(),
                        FieldsSelect::default(),
                        FieldsMultiSelect {
                            display: multiselect.display as usize,
                        },
                        options,
                    )
                }
            };

        ItemFields {
            id: item.id,
            name: item.name,
            name_touched: false,
            typ,
            text_type,
            checkbox_type,
            defaults,
            select_type,
            multi_select_type,
            options,
            validation_error: None,
        }
    });

    let validation_error = fields.read().validation_error.as_ref().map(|err| {
        rsx! {
            p {
                class: "help is-danger",
                "{err}"
            }
        }
    });
    let drag_line = drag_data.read().as_ref().and_then(|d: &DragData| d.line_location.as_ref()).map(|absolute_location| {
        rsx!{
            hr {
                style: "position: fixed; top: {absolute_location.top}px; left: {absolute_location.left}px; height: 5px; width: {absolute_location.width}px; margin-top: 0; margin-bottom: 0; background-color: black;",
            }
        }
    });

    rsx! { Modal {
        title: "New Field",
        onsubmit: move |_| {
            if fields.read().name.is_empty() {
                fields.with_mut(|fields| {
                    fields.name_touched = true;
                    fields.validation_error = Some("One or more fields have errors".to_owned())
                });
                return;
            }

            let item = fields.with(|fields| {
                RegistrationSchemaItem {
                    id: fields.id.clone(),
                    name: fields.name.clone(),
                    r#type: Some(RegistrationSchemaItemType{
                        r#type: Some(match type_selects.read()[fields.typ].0 {
                            ItemFieldsType::Text => ItemType::Text(TextType {
                                default: fields.text_type.default.clone(),
                                display: match text_display_selects.read()[fields.text_type.display].0 {
                                    TextDisplayType::Small => text_type::Display::Small,
                                    TextDisplayType::Large => text_type::Display::Large,
                                } as i32,
                            }),
                            ItemFieldsType::Checkbox => ItemType::Checkbox(CheckboxType {
                                default: fields.checkbox_type.default,
                            }),
                            ItemFieldsType::Select => ItemType::Select(SelectType{
                                default: fields.defaults.first().copied().unwrap_or(0) as u32,
                                display: match select_display_selects.read()[fields.select_type.display].0 {
                                    SelectDisplayType::Radio => select_type::Display::Radio,
                                    SelectDisplayType::Dropdown => select_type::Display::Dropdown,
                                } as i32,
                                options: fields.options.iter().map(|o| o.option.clone()).collect(),
                            }),
                            ItemFieldsType::MultiSelect => ItemType::MultiSelect(MultiSelectType{
                                defaults: fields.defaults.iter().map(|idx| *idx as u32).collect(),
                                display: match multi_select_display_selects.read()[fields.multi_select_type.display].0 {
                                    MultiSelectDisplayType::Checkboxes => multi_select_type::Display::Checkboxes,
                                    MultiSelectDisplayType::MultiselectBox => multi_select_type::Display::MultiselectBox,
                                } as i32,
                                options: fields.options.iter().map(|o| o.option.clone()).collect(),
                            }),
                        }),
                    }),
                }
            });

            onsubmit.call(item);
        },
        onclose: onclose,
        disable_submit: false,
        success_text: "{success_text}",
        form {
            Field {
                label: "Name",
                TextInput{
                    value: TextInputType::Text(fields.read().name.clone()),
                    oninput: move |evt: FormEvent| {
                        fields.with_mut(|fields| {
                            fields.name = evt.value();
                            fields.name_touched = false;
                            fields.validation_error = None;
                        })
                    },
                    onblur: move |_| fields.write().name_touched = true,
                    invalid: if fields.read().name_touched && fields.read().name.is_empty() {
                            Some("Name is required".to_owned())
                        } else {
                            None
                        },
                }
            }
            Field {
                label: "Type",
                SelectInput {
                    options: type_selects.read().iter().map(|(_, estr)| estr).cloned().collect::<Vec<String>>(),
                    onchange: {
                        move |evt: FormEvent| {
                            let idx = match evt.value().parse::<usize>() {
                                Ok(idx) => idx,
                                Err(e) => {
                                    toaster.write().new_error(format!("{}", e));
                                    return;
                                },
                            };
                            let requires_truncation = if matches!(type_selects.read()[idx].0, ItemFieldsType::Select) {
                                Some(fields.read().defaults.first().copied().unwrap_or(0))
                            } else {
                                None
                            };

                            fields.with_mut(|fields| {
                                fields.typ = idx;
                                if let Some(default) = requires_truncation {
                                    fields.defaults.clear();
                                    fields.defaults.insert(default);
                                }
                            })
                        }
                    },
                    value: fields.read().typ,
                }
            }
            div {
                class: "box",

                match type_selects.read()[fields.read().typ].0 {
                    ItemFieldsType::Text => rsx!(
                        Field {
                            label: "Default",
                            TextInput{
                                value: TextInputType::Text(fields.read().text_type.default.clone()),
                                oninput: move |evt: FormEvent| fields.write().text_type.default = evt.value(),
                            }
                        }
                        Field {
                            label: "Display",
                            SelectInput {
                                options: text_display_selects.read().iter().map(|(_, estr)| estr).cloned().collect::<Vec<String>>(),
                                onchange: {
                                    move |evt: FormEvent| {
                                        let idx = match evt.value().parse::<usize>() {
                                            Ok(idx) => idx,
                                            Err(e) => {
                                                toaster.write().new_error(format!("{}", e));
                                                return;
                                            },
                                        };
                                        fields.write().text_type.display = idx
                                    }
                                },
                                value: fields.read().text_type.display,
                            }
                        }
                    ),

                    ItemFieldsType::Checkbox => {
                        rsx!(
                            Field {
                                label: "Default",
                                CheckInput{
                                    style: CheckStyle::Checkbox,
                                    value: fields.read().checkbox_type.default,
                                    onclick: move |_| fields.with_mut(|fields| fields.checkbox_type.default = !fields.checkbox_type.default),
                                }
                            }
                        )
                    },

                    ItemFieldsType::Select => rsx!(
                        Field {
                            label: "Display",
                            SelectInput {
                                options: select_display_selects.read().iter().map(|(_, estr)| estr).cloned().collect::<Vec<String>>(),
                                onchange: {
                                    move |evt: FormEvent| {
                                        let idx = match evt.value().parse::<usize>() {
                                            Ok(idx) => idx,
                                            Err(e) => {
                                                toaster.write().new_error(format!("{}", e));
                                                return;
                                            },
                                        };
                                        fields.write().select_type.display = idx
                                    }
                                },
                                value: fields.read().select_type.display,
                            }
                        }
                    { fields.read().options.iter().enumerate().map(|(idx, option)| {
                            let key = option.key;
                            let prev_key = idx.checked_sub(1).and_then(|idx| fields.read().options.get(idx).map(|o| o.key));
                            let next_key = fields.read().options.get(idx + 1).map(|o| o.key);
                            rsx!{
                                Field {
                                    onmounted: move |d: MountedEvent| { field_refs.write().insert(key, d.data); },
                                    ondragover: move |dragover: DragEvent| {
                                        to_owned!(field_refs, drag_data, toaster);
                                        spawn(async move {
                                            ondragover(dragover, drag_data, field_refs, toaster, idx, key, prev_key, next_key).await;
                                        });
                                    },
                                    key: "{key}",
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].option.name = evt.value(),
                                    }
                                    CheckInput{
                                        style: CheckStyle::Radio,
                                        label: "Default",
                                        value: fields.read().defaults.contains(&idx),
                                        onclick: move |_| {
                                            fields.with_mut(|fields| {
                                                fields.defaults.clear();
                                                fields.defaults.insert(idx);
                                            })
                                        }
                                    }
                                    {
                                        rsx! {
                                            div {
                                                class: "field",
                                                OptionGrab {
                                                    idx: idx,
                                                    drag_data: drag_data,
                                                    grabbing_cursor: grabbing_cursor,
                                                    ondragend: move |data: DragData| {
                                                        if data.dragged < data.new_location {
                                                            fields.write().options[data.dragged..=data.new_location].rotate_left(1);
                                                        } else {
                                                            fields.write().options[data.new_location..=data.dragged].rotate_right(1);
                                                        };
                                                    },
                                                }
                                            }
                                        }
                                    }
                                    div {
                                        class: "field",
                                        button {
                                            class: "delete",
                                            "type": "button",
                                            "aria-label": "close",
                                            onclick: move |_| {
                                                fields.write().options.remove(idx);
                                            },
                                        }
                                    }
                                }
                            }
                        }) }
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: move |_| fields.write().options.push(FieldSelectOption::default()),
                            "Add Option"
                        }
                    ),
                    ItemFieldsType::MultiSelect => rsx!{
                        Field {
                            label: "Display",
                            SelectInput {
                                options: multi_select_display_selects.read().iter().map(|(_, estr)| estr).cloned().collect::<Vec<String>>(),
                                onchange: {
                                    to_owned!(toaster);
                                    move |evt: FormEvent| {
                                        let idx = match evt.value().parse::<usize>() {
                                            Ok(idx) => idx,
                                            Err(e) => {
                                                toaster.write().new_error(format!("{}", e));
                                                return;
                                            },
                                        };
                                        fields.write().multi_select_type.display = idx
                                    }
                                },
                                value: fields.read().multi_select_type.display,
                            }
                        }
                        Field {
                            label: "",
                            Button {
                                flavor: ButtonFlavor::Info,
                                onclick: move |_| fields.write().defaults.clear(),
                                "Clear Defaults",
                            }
                        }
                        { fields.read().options.iter().enumerate().map(|(idx, option)| {
                            rsx!{
                                Field {
                                    key: "{option.key}",
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].option.name = evt.value().clone(),
                                    }
                                    CheckInput{
                                        style: CheckStyle::Checkbox,
                                        label: "Default",
                                        value: fields.read().defaults.contains(&idx),
                                        onclick: move |_| {
                                            fields.with_mut(|fields| {
                                                if !fields.defaults.remove(&idx) {
                                                    fields.defaults.insert(idx);
                                                };
                                            })
                                        }
                                    }
                                    {
                                        rsx! {
                                            div {
                                                class: "field",
                                                OptionGrab {
                                                    idx: idx,
                                                    drag_data: drag_data,
                                                    grabbing_cursor: grabbing_cursor,
                                                    ondragend: move |data: DragData| {
                                                        if data.dragged < data.new_location {
                                                            fields.write().options[data.dragged..=data.new_location].rotate_left(1);
                                                        } else {
                                                            fields.write().options[data.new_location..=data.dragged].rotate_right(1);
                                                        };
                                                    },
                                                }
                                            }
                                        }
                                    }
                                    div {
                                        class: "field",
                                        button {
                                            class: "delete",
                                            "type": "button",
                                            "aria-label": "close",
                                            onclick: move |_| {
                                                fields.write().options.remove(idx);
                                            },
                                        }
                                    }
                                }
                            }
                        }) }
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: move |_| fields.write().options.push(FieldSelectOption::default()),
                            "Add Option"
                        }
                    },
                }
            }
            { validation_error }
            { drag_line }
        }
    }}
}

#[component]
fn DeleteItemModal(onsubmit: EventHandler<()>, onclose: EventHandler<()>) -> Element {
    rsx!(Modal {
        title: "Delete Field",
        onsubmit: onsubmit,
        onclose: onclose,
        disable_submit: false,
        success_text: "Delete",
        p {
            "Are you sure you want to delete this field?"
        }
    })
}

async fn ondragover(
    dragover: DragEvent,
    mut drag_data: Signal<Option<DragData>>,
    dom_refs: Signal<HashMap<Uuid, Rc<MountedData>>>,
    mut toaster: Signal<ToastManager>,
    idx: usize,
    key: Uuid,
    prev_key: Option<Uuid>,
    next_key: Option<Uuid>,
) {
    if drag_data.read().is_none() {
        return;
    }

    let row_refs = dom_refs.read();
    let row_ref = match row_refs.get(&key) {
        Some(row_ref) => row_ref,
        None => return,
    };

    let prev_ref = match prev_key {
        Some(prev_key) => match row_refs.get(&prev_key) {
            Some(prev_ref) => Some(prev_ref),
            None => return,
        },
        None => None,
    };

    let next_ref = match next_key {
        Some(next_key) => match row_refs.get(&next_key) {
            Some(next_ref) => Some(next_ref),
            None => return,
        },
        None => None,
    };

    let rect_future = row_ref.get_client_rect();
    let prev_rect_future = async {
        match prev_ref {
            Some(prev_ref) => Some(prev_ref.get_client_rect().await),
            None => None,
        }
    };
    let next_rect_future = async {
        match next_ref {
            Some(next_ref) => Some(next_ref.get_client_rect().await),
            None => None,
        }
    };

    let (rect, prev_rect, next_rect) = join! {
        rect_future,
        prev_rect_future,
        next_rect_future,
    };

    let rect = match rect {
        Ok(rect) => rect,
        Err(e) => {
            toaster.write().new_error(e.to_string());
            return;
        }
    };

    let prev_rect = match prev_rect.transpose() {
        Ok(rect) => rect,
        Err(e) => {
            toaster.write().new_error(e.to_string());
            return;
        }
    };

    let next_rect = match next_rect.transpose() {
        Ok(rect) => rect,
        Err(e) => {
            toaster.write().new_error(e.to_string());
            return;
        }
    };

    let new_drag_data = drag_data.read().as_ref().map(|line_location| {
        let (new_location, top) =
            if dragover.page_coordinates().y < rect.min_y() + rect.height() / 2.0 {
                let new_location = if line_location.dragged < idx {
                    idx - 1
                } else {
                    idx
                };

                let position = match prev_rect {
                    Some(prev_rect) => (prev_rect.max_y() + rect.min_y()) / 2.0,
                    None => rect.min_y(),
                };

                (new_location, position)
            } else {
                let new_location = if line_location.dragged <= idx {
                    idx
                } else {
                    idx + 1
                };

                let position = match next_rect {
                    Some(next_rect) => (next_rect.min_y() + rect.max_y()) / 2.0,
                    None => rect.max_y(),
                };

                (new_location, position)
            };

        DragData {
            dragged: line_location.dragged,
            new_location,
            line_location: Some(LineLocation {
                top,
                left: rect.min_x(),
                width: rect.width(),
            }),
        }
    });

    if let Some(new_drag_data) = new_drag_data {
        drag_data.set(Some(new_drag_data));
    };
}

#[component]
fn OptionGrab(
    idx: usize,
    drag_data: Signal<Option<DragData>>,
    grabbing_cursor: Signal<bool>,
    ondragend: EventHandler<DragData>,
) -> Element {
    let style = if *grabbing_cursor.read() {
        "grabbing"
    } else {
        "grab"
    };

    rsx! {
        div{
            style: "width: 1px",
            draggable: true,
            onmousedown: move |_| grabbing_cursor.set(true),
            onmouseup: move |_| grabbing_cursor.set(false),
            ondragstart: move |_| {
                drag_data.set(Some(DragData{
                    dragged: idx,
                    new_location: idx,
                    line_location: None,
                }));
            },
            ondragend: move |_| {
                grabbing_cursor.set(false);
                let data = match drag_data().clone() {
                    Some(d) => d,
                    None => return,
                };

                drag_data.set(None);

                if data.dragged == data.new_location {
                    return;
                }

                ondragend.call(data);
            },
            cursor: "{style}",
            "⣶",
        }
    }
}

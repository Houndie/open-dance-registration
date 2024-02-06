use std::collections::BTreeSet;

use crate::{
    components::{
        form::{
            Button, ButtonFlavor, CheckInput, CheckStyle, Field, SelectInput, TextInput,
            TextInputType,
        },
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};
use common::proto::{
    event_query, multi_select_type, registration_schema_item_type::Type as ItemType,
    registration_schema_query, select_type, string_query, text_type, CheckboxType, EventQuery,
    MultiSelectType, QueryEventsRequest, QueryRegistrationSchemasRequest, RegistrationSchema,
    RegistrationSchemaItem, RegistrationSchemaItemType, RegistrationSchemaQuery, SelectOption,
    SelectType, StringQuery, TextType, UpsertRegistrationSchemasRequest,
};
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
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

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    let show_schema_item_modal = use_state(cx, || None);

    let toaster = use_toasts(cx).unwrap();
    let nav = use_navigator(cx);
    let grpc_client = use_grpc_client(cx).unwrap();

    let event = use_future(cx, (), |_| {
        to_owned!(grpc_client, id, nav, toaster);
        async move {
            let result = grpc_client
                .events
                .query_events(tonic::Request::new(QueryEventsRequest {
                    query: Some(EventQuery {
                        query: Some(event_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(id)),
                        })),
                    }),
                }))
                .await;

            let response = match result {
                Ok(rsp) => rsp,
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    return None;
                }
            };

            let event = response.into_inner().events.pop();

            if event.is_none() {
                nav.push(Routes::NotFound);
                return None;
            }

            event
        }
    });

    let _event = match event.value().map(|e| e.as_ref()).flatten() {
        Some(event) => event,
        None => return None,
    };

    let schema = use_ref(cx, Schema::default);

    let schema_loaded = use_future(cx, (), |_| {
        to_owned!(grpc_client, id, toaster, schema);
        async move {
            let result = grpc_client
                .registration_schema
                .query_registration_schemas(tonic::Request::new(QueryRegistrationSchemasRequest {
                    query: Some(RegistrationSchemaQuery {
                        query: Some(registration_schema_query::Query::EventId(StringQuery {
                            operator: Some(string_query::Operator::Equals(id.clone())),
                        })),
                    }),
                }))
                .await;

            let response = match result {
                Ok(rsp) => rsp,
                Err(e) => {
                    toaster.write().new_error(e.to_string());
                    return false;
                }
            };

            let registration_schema = response
                .into_inner()
                .registration_schemas
                .pop()
                .unwrap_or_else(|| {
                    let mut schema = RegistrationSchema::default();
                    schema.event_id = id;
                    schema
                });

            *schema.write() = Schema {
                items: registration_schema.items.iter().map(|item| (Uuid::new_v4(), item.clone())).collect(),
                event_id: registration_schema.event_id,
            };

            true
        }
    });

    if !schema_loaded.value().unwrap_or(&false) {
        return None;
    }

    cx.render(rsx! {
        GenericPage {
            title: "Modify Registration Schema".to_owned(),
            Table {
                is_striped: true,
                is_fullwidth: true,
                thead {
                    tr {
                        th {
                            class: "col-auto",
                            "Item"
                        }
                        th{
                            style: "width: 1px",
                        }
                    }
                }
                tbody {
                    schema.read().items.iter().map(|(key, i)| {
                        let item = i.clone();
                        let key = key.clone();

                        let show_schema_item_modal = show_schema_item_modal.clone();
                        rsx!{
                            tr {
                                key: "{key}",
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
                            }
                        }
                    })
                }
            }

            Button {
                flavor: ButtonFlavor::Info,
                onclick: |_| show_schema_item_modal.set(Some((Uuid::new_v4(), default_registration_schema_item()))),
                "Add Field"
            }
        }
        if let Some((key, item)) = show_schema_item_modal.get() {
            rsx!(NewSchemaItemModal{
                initial: item.clone(),
                do_submit: |item| {
                    let mut send_schema = schema.read().clone();
                    if item.id == "" {
                        send_schema.items.push((key.clone(), item.clone()));
                    } else {
                        let idx = send_schema.items.iter().position(|(_, i)| i.id == item.id);
                        match idx {
                            Some(idx) => send_schema.items[idx] = (key.clone(), item.clone()),
                            None => {
                                toaster.write().new_error("Item not found".to_owned());
                                return;
                            },
                        }
                    }

                    let registration_schema = RegistrationSchema {
                        event_id: send_schema.event_id.clone(),
                        items: send_schema.items.iter().cloned().map(|(_, i)| i).collect(),
                    };

                    cx.spawn({
                        to_owned!(grpc_client, toaster, registration_schema);
                        async move { 
                            let rsp = grpc_client.registration_schema.upsert_registration_schemas(UpsertRegistrationSchemasRequest{
                                registration_schemas: vec![registration_schema],
                            }).await;

                            match rsp {
                                Ok(_) => {},
                                Err(e) => {
                                    toaster.write().new_error(e.to_string());
                                }
                            }
                        }
                    });
                    *schema.write() = send_schema;
                    show_schema_item_modal.set(None);
                },
                do_close: || show_schema_item_modal.set(None),
            })
        }
    })
}

#[derive(EnumIter, strum_macros::Display)]
enum ItemFieldsType {
    Text,
    Checkbox,
    Select,
    MultiSelect,
}

#[derive(EnumIter, strum_macros::Display)]
enum TextDisplayType {
    Small,
    Large,
}

#[derive(EnumIter, strum_macros::Display)]
enum SelectDisplayType {
    Radio,
    Dropdown,
}

#[derive(EnumIter, strum_macros::Display)]
enum MultiSelectDisplayType {
    Checkboxes,
    MultiselectBox,
}

#[derive(Default)]
struct FieldsSelect {
    display: usize,
}

#[derive(Default)]
struct FieldsMultiSelect {
    display: usize,
}

#[derive(Default)]
struct FieldsText {
    default: String,
    display: usize,
}

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
    options: Vec<SelectOption>,
    validation_error: Option<String>,
}

impl Default for ItemFields {
    fn default() -> Self {
        ItemFields {
            id: String::default(),
            name: String::default(),
            name_touched: false,
            typ: 0,
            text_type: FieldsText::default(),
            checkbox_type: CheckboxType::default(),
            select_type: FieldsSelect::default(),
            multi_select_type: FieldsMultiSelect::default(),
            options: Vec::default(),
            defaults: BTreeSet::default(),
            validation_error: None,
        }
    }
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
fn NewSchemaItemModal<DoSubmit: Fn(RegistrationSchemaItem) -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    initial: RegistrationSchemaItem,
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let toaster = use_toasts(cx).unwrap();
    let type_selects = use_const(cx, || enum_selects::<ItemFieldsType>());
    let text_display_selects = use_const(cx, || enum_selects::<TextDisplayType>());
    let select_display_selects = use_const(cx, || enum_selects::<SelectDisplayType>());
    let multi_select_display_selects = use_const(cx, || enum_selects::<MultiSelectDisplayType>());
    let fields = use_ref(cx, || {
        let item = initial.clone();

        let (typ, text_type, checkbox_type, defaults, select_type, multi_select_type, options) = match item.r#type.unwrap().r#type.unwrap() {
            ItemType::Text(text) => (0, FieldsText {
                default: text.default,
                display: text.display as usize,
            }, CheckboxType::default(), BTreeSet::default(), FieldsSelect::default(), FieldsMultiSelect::default(), Vec::default()),
            ItemType::Checkbox(checkbox) => (1, FieldsText::default(), checkbox, BTreeSet::default(), FieldsSelect::default(), FieldsMultiSelect::default(), Vec::default()),
            ItemType::Select(select) => (2, FieldsText::default(), CheckboxType::default(), BTreeSet::from([select.default as usize]), FieldsSelect {
                display: select.display as usize,
            }, FieldsMultiSelect::default(), select.options),
            ItemType::MultiSelect(multiselect) => (3, FieldsText::default(), CheckboxType::default(), multiselect.defaults.into_iter().map(|d| d as usize).collect(), FieldsSelect::default(), FieldsMultiSelect{
                display: multiselect.display as usize,
            }, multiselect.options),
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

    cx.render(rsx!(Modal {
        title: "New Field",
        do_submit: || {
            if fields.read().name == "" {
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
                        r#type: Some(match type_selects[fields.typ].0 {
                            ItemFieldsType::Text => ItemType::Text(TextType {
                                default: fields.text_type.default.clone(),
                                display: match text_display_selects[fields.text_type.display].0 {
                                    TextDisplayType::Small => text_type::Display::Small,
                                    TextDisplayType::Large => text_type::Display::Large,
                                } as i32,
                            }),
                            ItemFieldsType::Checkbox => ItemType::Checkbox(CheckboxType {
                                default: fields.checkbox_type.default,
                            }),
                            ItemFieldsType::Select => ItemType::Select(SelectType{
                                default: fields.defaults.first().copied().unwrap_or(0) as u32,
                                display: match select_display_selects[fields.select_type.display].0 {
                                    SelectDisplayType::Radio => select_type::Display::Radio,
                                    SelectDisplayType::Dropdown => select_type::Display::Dropdown,
                                } as i32,
                                options: fields.options.clone(),
                            }),
                            ItemFieldsType::MultiSelect => ItemType::MultiSelect(MultiSelectType{
                                defaults: fields.defaults.iter().map(|idx| *idx as u32).collect(),
                                display: match multi_select_display_selects[fields.multi_select_type.display].0 {
                                    MultiSelectDisplayType::Checkboxes => multi_select_type::Display::Checkboxes,
                                    MultiSelectDisplayType::MultiselectBox => multi_select_type::Display::MultiselectBox,
                                } as i32,
                                options: fields.options.clone(),
                            }),
                        }),
                    }),
                }
            });

            do_submit(item);
        },
        do_close: do_close,
        disable_submit: false,
        form {
            Field {
                label: "Name",
                TextInput{
                    value: TextInputType::Text(fields.read().name.clone()),
                    oninput: |evt: FormEvent| {
                        fields.with_mut(|fields| {
                            fields.name = evt.value.clone();
                            fields.name_touched = false;
                            fields.validation_error = None;
                        })
                    },
                    onblur: |_| fields.write().name_touched = true,
                    invalid: if fields.read().name_touched && fields.read().name == "" {
                            Some("Name is required".to_owned())
                        } else {
                            None
                        },
                }
            }
            Field {
                label: "Type",
                SelectInput {
                    options: type_selects.iter().map(|(_, estr)| estr).cloned().collect(),
                    onchange: {
                        let toaster = toaster.clone();
                        move |evt: FormEvent| {
                            let idx = match evt.value.parse::<usize>() {
                                Ok(idx) => idx,
                                Err(e) => {
                                    toaster.write().new_error(format!("{}", e));
                                    return;
                                },
                            };
                            let requires_truncation = if matches!(type_selects[idx].0, ItemFieldsType::Select) {
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

                match type_selects[fields.read().typ].0 {
                    ItemFieldsType::Text => rsx!(
                        Field {
                            label: "Default",
                            TextInput{
                                value: TextInputType::Text(fields.read().text_type.default.clone()),
                                oninput: |evt: FormEvent| fields.write().text_type.default = evt.value.clone(),
                            }
                        }
                        Field {
                            label: "Display",
                            SelectInput {
                                options: text_display_selects.iter().map(|(_, estr)| estr).cloned().collect(),
                                onchange: {
                                    let toaster = toaster.clone();
                                    move |evt: FormEvent| {
                                        let idx = match evt.value.parse::<usize>() {
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
                                    onclick: |_| fields.with_mut(|fields| fields.checkbox_type.default = !fields.checkbox_type.default),
                                }
                            }
                        )
                    },

                    ItemFieldsType::Select => rsx!(
                        Field {
                            label: "Display",
                            SelectInput {
                                options: select_display_selects.iter().map(|(_, estr)| estr).cloned().collect(),
                                onchange: {
                                    let toaster = toaster.clone();
                                    move |evt: FormEvent| {
                                        let idx = match evt.value.parse::<usize>() {
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
                        fields.read().options.iter().enumerate().map(|(idx, option)| {
                            rsx!{
                                Field {
                                    key: "{idx}",
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].name = evt.value.clone(),
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
                                }
                            }
                        })
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: |_| fields.write().options.push(SelectOption::default()),
                            "Add Option"
                        }
                    ),
                    ItemFieldsType::MultiSelect => rsx!{
                        Field {
                            label: "Display",
                            SelectInput {
                                options: multi_select_display_selects.iter().map(|(_, estr)| estr).cloned().collect(),
                                onchange: {
                                    to_owned!(toaster);
                                    move |evt: FormEvent| {
                                        let idx = match evt.value.parse::<usize>() {
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
                                onclick: |_| fields.write().defaults.clear(),
                                "Clear Defaults",
                            }
                        }
                        fields.read().options.iter().enumerate().map(|(idx, option)| {
                            rsx!{
                                Field {
                                    key: "{idx}",
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].name = evt.value.clone(),
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
                                }
                            }
                        })
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: |_| fields.write().options.push(SelectOption::default()),
                            "Add Option"
                        }
                    },
                }
            }
            if let Some(err) = fields.read().validation_error.as_ref() {
                rsx!{
                    p {
                        class: "help is-danger",
                        "{err}"
                    }
                }
            }
        }
    }))
}

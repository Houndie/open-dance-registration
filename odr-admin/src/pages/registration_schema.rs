use std::{collections::{BTreeSet, HashMap}, rc::Rc};

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
    hooks::{toasts::{use_toasts, ToastManager}, use_grpc_client},
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
use futures::{join, Future};
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
pub fn Page(cx: Scope, id: String) -> Element {
    let show_schema_item_modal = use_state(cx, || None);
    let show_delete_item_modal = use_state(cx, || None);

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

    let table_row_refs: &UseRef<HashMap<Uuid, Rc<MountedData>>> = use_ref(cx, HashMap::default);
    let drag_data: &UseState<Option<DragData>> = use_state(cx, || None);
    let grabbing_cursor = use_state(cx, || false);
    let cursor = if *grabbing_cursor.get() {
        "cursor: grabbing; cursor: -moz-grabbing; cursor: -webkit-grabbing;"
    } else {
        "cursor: auto"
    }.to_owned();

    cx.render(rsx! {
        GenericPage {
            title: "Modify Registration Schema".to_owned(),
            style: cursor,
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
                    schema.read().items.iter().enumerate().map(|(idx, (key, i))| {
                        let item = i.clone();
                        let key = key.clone();

                        let show_schema_item_modal = show_schema_item_modal.clone();
                        rsx!{
                            tr {
                                key: "{key}",
                                onmounted: move |d| { table_row_refs.write().insert(key, d.data); },
                                ondragover: move |dragover| {
                                    to_owned!(table_row_refs, drag_data, toaster);
                                    cx.spawn(async move {
                                        ondragover(dragover, drag_data.clone(), table_row_refs.clone(), toaster, idx, key, None, None).await;
                                    });
                                },
                                td {
                                    {
                                        to_owned!(schema, toaster, grpc_client);
                                        rsx! {
                                            OptionGrab {
                                                drag_data: drag_data.clone(),
                                                grabbing_cursor: grabbing_cursor.clone(),
                                                idx: idx,
                                                do_dragend: move |data| {
                                                    to_owned!(schema, data, grpc_client, toaster);
                                                    async move {
                                                        let mut schema_copy = schema.read().clone();
                                                        if data.dragged < data.new_location {
                                                            schema_copy.items[data.dragged..=data.new_location].rotate_left(1);
                                                        } else {
                                                            schema_copy.items[data.new_location..=data.dragged].rotate_right(1);
                                                        }

                                                        *schema.write() = schema_copy.clone();


                                                        let res = grpc_client.registration_schema.upsert_registration_schemas(UpsertRegistrationSchemasRequest{
                                                            registration_schemas: vec![schema_copy.into()],
                                                        }).await;

                                                        if let Err(e) = res {
                                                            toaster.write().new_error(e.to_string());
                                                        }
                                                    }
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
                    })
                }
            }

            Button {
                flavor: ButtonFlavor::Info,
                onclick: |_| show_schema_item_modal.set(Some((Uuid::new_v4(), default_registration_schema_item()))),
                "Add Field"
            }
        }
        if let Some(absolute_location) = drag_data.get().as_ref().and_then(|d| d.line_location.as_ref()) {
            rsx!{
                hr {
                    style: "position: fixed; top: {absolute_location.top}px; left: {absolute_location.left}px; height: 5px; width: {absolute_location.width}px; margin-top: 0; margin-bottom: 0; background-color: black;",
                }
            }
        }
        if let Some((key, item)) = show_schema_item_modal.get() {
            rsx!(NewSchemaItemModal{
                initial: item.clone(),
                grabbing_cursor: grabbing_cursor.clone(),
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
        if let Some(idx) = show_delete_item_modal.get() {
            rsx!(DeleteItemModal{
                do_submit: move || {
                    let registration_schema = RegistrationSchema {
                        event_id: schema.read().event_id.clone(),
                        items: schema.read().items.iter().enumerate().filter(|(i, _)| i != idx).map(|(_, (_, i))| i).cloned().collect(),
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

                    schema.write().items.remove(*idx);
                    table_row_refs.write().remove(&schema.read().items[*idx].0);
                    show_delete_item_modal.set(None);
                },
                do_close: || show_delete_item_modal.set(None),
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

#[derive(Default, Clone, Debug)]
struct FieldSelectOption {
    key: Uuid,
    option: SelectOption,
}

#[derive(Clone)]
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
    grabbing_cursor: UseState<bool>,
) -> Element {
    let toaster = use_toasts(cx).unwrap();
    let type_selects = use_const(cx, || enum_selects::<ItemFieldsType>());
    let text_display_selects = use_const(cx, || enum_selects::<TextDisplayType>());
    let select_display_selects = use_const(cx, || enum_selects::<SelectDisplayType>());
    let multi_select_display_selects = use_const(cx, || enum_selects::<MultiSelectDisplayType>());
    let drag_data: &UseState<Option<DragData>> = use_state(cx, || None);
    let field_refs: &UseRef<HashMap<Uuid, Rc<MountedData>>> = use_ref(cx, HashMap::default);

    let fields = use_ref(cx, || {
        let item = initial.clone();

        let (typ, text_type, checkbox_type, defaults, select_type, multi_select_type, options) = match item.r#type.unwrap().r#type.unwrap() {
            ItemType::Text(text) => (0, FieldsText {
                default: text.default,
                display: text.display as usize,
            }, CheckboxType::default(), BTreeSet::default(), FieldsSelect::default(), FieldsMultiSelect::default(), Vec::default()),
            ItemType::Checkbox(checkbox) => (1, FieldsText::default(), checkbox, BTreeSet::default(), FieldsSelect::default(), FieldsMultiSelect::default(), Vec::default()),
            ItemType::Select(select) => {
                let options = select.options.into_iter().map(|option| {
                    let key = Uuid::new_v4();
                    FieldSelectOption {
                        key,
                        option,
                    }
                }).collect();

                (2, FieldsText::default(), CheckboxType::default(), BTreeSet::from([select.default as usize]), FieldsSelect {
                display: select.display as usize,
            }, FieldsMultiSelect::default(), options)
            },
            ItemType::MultiSelect(multiselect) => {
                let options = multiselect.options.into_iter().map(|option| {
                    let key = Uuid::new_v4();
                    FieldSelectOption {
                        key,
                        option,
                    }
                }).collect();
                (3, FieldsText::default(), CheckboxType::default(), multiselect.defaults.into_iter().map(|d| d as usize).collect(), FieldsSelect::default(), FieldsMultiSelect{
                display: multiselect.display as usize,
            }, options)
            },
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
                                options: fields.options.iter().map(|o| o.option.clone()).collect(),
                            }),
                            ItemFieldsType::MultiSelect => ItemType::MultiSelect(MultiSelectType{
                                defaults: fields.defaults.iter().map(|idx| *idx as u32).collect(),
                                display: match multi_select_display_selects[fields.multi_select_type.display].0 {
                                    MultiSelectDisplayType::Checkboxes => multi_select_type::Display::Checkboxes,
                                    MultiSelectDisplayType::MultiselectBox => multi_select_type::Display::MultiselectBox,
                                } as i32,
                                options: fields.options.iter().map(|o| o.option.clone()).collect(),
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
                            let key = option.key;
                            let prev_key = idx.checked_sub(1).and_then(|idx| fields.read().options.get(idx).map(|o| o.key));
                            let next_key = fields.read().options.get(idx + 1).map(|o| o.key);
                            rsx!{
                                Field {
                                    onmounted: move |d: MountedEvent| { field_refs.write().insert(key, d.data); },
                                    ondragover: move |dragover: DragEvent| {
                                        to_owned!(field_refs, drag_data, toaster);
                                        cx.spawn(async move {
                                            ondragover(dragover, drag_data.clone(), field_refs.clone(), toaster, idx, key, prev_key, next_key).await;
                                        });
                                    },
                                    key: "{key}",
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].option.name = evt.value.clone(),
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
                                        let fields = fields.clone();
                                        rsx! {
                                            div {
                                                class: "field",
                                                OptionGrab {
                                                    idx: idx,
                                                    drag_data: drag_data.clone(),
                                                    grabbing_cursor: grabbing_cursor.clone(),
                                                    do_dragend: move |data| {
                                                        to_owned!(fields, data);
                                                        async move {
                                                            if data.dragged < data.new_location {
                                                                fields.write().options[data.dragged..=data.new_location].rotate_left(1);
                                                            } else {
                                                                fields.write().options[data.new_location..=data.dragged].rotate_right(1);
                                                            };
                                                        }
                                                    },
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        })
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: |_| fields.write().options.push(FieldSelectOption::default()),
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
                                    key: "{option.key}",
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].option.name = evt.value.clone(),
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
                            onclick: |_| fields.write().options.push(FieldSelectOption::default()),
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
            if let Some(absolute_location) = drag_data.get().as_ref().and_then(|d| d.line_location.as_ref()) {
                rsx!{
                    hr {
                        style: "position: fixed; top: {absolute_location.top}px; left: {absolute_location.left}px; height: 5px; width: {absolute_location.width}px; margin-top: 0; margin-bottom: 0; background-color: black;",
                    }
                }
            }
        }
    }))
}

#[component]
fn DeleteItemModal<DoSubmit: Fn() -> (), DoClose: Fn() -> ()>(cx: Scope, do_submit: DoSubmit, do_close: DoClose) -> Element {

    cx.render(rsx!(Modal {
        title: "Delete Field",
        do_submit: do_submit,
        do_close: do_close,
        disable_submit: false,
        p {
            "Are you sure you want to delete this field?"
        }
    }))
}

async fn ondragover(dragover: DragEvent, drag_data: UseState<Option<DragData>>, dom_refs: UseRef<HashMap<Uuid, Rc<MountedData>>>, toaster: UseSharedState<ToastManager>, idx: usize, key: Uuid, prev_key: Option<Uuid>, next_key: Option<Uuid>) {
    if drag_data.get().is_none() {
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

    let (rect, prev_rect, next_rect) = join!{
        rect_future,
        prev_rect_future,
        next_rect_future,
    };

    let rect = match rect {
        Ok(rect) => rect,
        Err(e) => {
            toaster.write().new_error(e.to_string());
            return
        },
    };

    let prev_rect = match prev_rect.transpose() {
        Ok(rect) => rect,
        Err(e) => {
            toaster.write().new_error(e.to_string());
            return
        },
    };

    let next_rect = match next_rect.transpose() {
        Ok(rect) => rect,
        Err(e) => {
            toaster.write().new_error(e.to_string());
            return
        },
    };

    if let Some(line_location) = drag_data.get() {
        let (new_location, top) = if dragover.mouse.page_coordinates().y < rect.min_y() + rect.height() / 2.0 {
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

        drag_data.set(Some(DragData{
            dragged: line_location.dragged,
            new_location,
            line_location: Some(LineLocation{
                top,
                left: rect.min_x(),
                width: rect.width(),
            }),
        }));
    }
}

#[derive(Props)]
struct OptionGrabProps<F, Fut> 
    where F: Fn(&DragData) -> Fut + Clone + 'static, Fut: Future<Output = ()> + 'static,
{
    idx: usize,
    drag_data: UseState<Option<DragData>>,
    grabbing_cursor: UseState<bool>,
    do_dragend: F,
}

fn OptionGrab<F, Fut>(cx: Scope<OptionGrabProps<F, Fut>>) -> Element
    where F: Fn(&DragData) -> Fut + Clone + 'static, Fut: Future<Output = ()> + 'static,
{
    let style = if *cx.props.grabbing_cursor.get() {
        "grabbing"
    } else {
        "grab"
    };

    cx.render(rsx!{
        div{
            style: "width: 1px",
            draggable: true,
            onmousedown: |_| cx.props.grabbing_cursor.set(true),
            onmouseup: |_| cx.props.grabbing_cursor.set(false),
            ondragstart: move |_| {
                cx.props.drag_data.set(Some(DragData{
                    dragged: cx.props.idx,
                    new_location: cx.props.idx,
                    line_location: None,
                }));
            },
            ondragend: move |_| {
                cx.props.grabbing_cursor.set(false);
                cx.spawn({
                    to_owned!(cx.props.drag_data, cx.props.do_dragend);
                    async move {
                        let data = match drag_data.get() {
                            Some(d) => d,
                            None => return,
                        };

                        drag_data.set(None);

                        if data.dragged == data.new_location {
                            return;
                        }

                        (do_dragend)(data).await;
                    }
                });
            },
            cursor: "{style}",
            "⣶",
        }
    })
}

use std::collections::{BTreeSet, HashMap};

use common::proto::{
    event_query, registration_query, registration_schema_item_type, registration_schema_query,
    string_query, EventQuery, QueryEventsRequest, QueryRegistrationSchemasRequest,
    QueryRegistrationsRequest, Registration, RegistrationItem, RegistrationQuery,
    RegistrationSchema, RegistrationSchemaQuery, StringQuery, UpsertRegistrationsRequest,
};
use dioxus::prelude::*;
use dioxus_router::hooks::use_navigator;

use crate::{
    components::{
        form::{
            Button, ButtonFlavor, CheckInput, CheckStyle, Field, MultiSelectInput, SelectInput,
            TextInput, TextInputType,
        },
        modal::Modal,
        page::Page as GenericPage,
        table::Table,
    },
    hooks::{toasts::use_toasts, use_grpc_client},
    pages::Routes,
};

#[derive(Default, Clone)]
struct TableRegistration {
    id: String,
    items: HashMap<String, String>,
}

impl From<Registration> for TableRegistration {
    fn from(registration: Registration) -> Self {
        Self {
            id: registration.id,
            items: registration
                .items
                .into_iter()
                .map(|item| (item.schema_item_id, item.value))
                .collect(),
        }
    }
}

fn to_proto_registration(registration: TableRegistration, event_id: String) -> Registration {
    Registration {
        id: registration.id,
        event_id,
        items: registration
            .items
            .into_iter()
            .map(|(schema_item_id, value)| RegistrationItem {
                schema_item_id,
                value,
            })
            .collect(),
    }
}

#[component]
pub fn Page(cx: Scope, event_id: String) -> Element {
    let grpc_client = use_grpc_client(cx).unwrap();
    let toaster = use_toasts(cx).unwrap();
    let nav = use_navigator(cx);

    let event_found = use_future(cx, (), |_| {
        to_owned!(grpc_client, event_id, nav, toaster);
        async move {
            let result = grpc_client
                .events
                .query_events(tonic::Request::new(QueryEventsRequest {
                    query: Some(EventQuery {
                        query: Some(event_query::Query::Id(StringQuery {
                            operator: Some(string_query::Operator::Equals(event_id)),
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

            if response.into_inner().events.len() == 0 {
                nav.push(Routes::NotFound);
                return false;
            }

            true
        }
    });

    if !event_found.value().unwrap_or(&false) {
        return None;
    };

    let schema = use_future(cx, (), |_| {
        to_owned!(grpc_client, event_id, toaster);
        async move {
            let result = grpc_client
                .registration_schema
                .query_registration_schemas(tonic::Request::new(QueryRegistrationSchemasRequest {
                    query: Some(RegistrationSchemaQuery {
                        query: Some(registration_schema_query::Query::EventId(StringQuery {
                            operator: Some(string_query::Operator::Equals(event_id.clone())),
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

            Some(
                response
                    .into_inner()
                    .registration_schemas
                    .pop()
                    .unwrap_or_else(|| {
                        let mut schema = RegistrationSchema::default();
                        schema.event_id = event_id;
                        schema
                    }),
            )
        }
    });

    let registrations: &UseRef<Vec<TableRegistration>> = use_ref(cx, Vec::new);

    let registrations_loaded = use_future(cx, (), |_| {
        to_owned!(grpc_client, event_id, toaster, registrations);
        async move {
            let result = grpc_client
                .registration
                .query_registrations(tonic::Request::new(QueryRegistrationsRequest {
                    query: Some(RegistrationQuery {
                        query: Some(registration_query::Query::EventId(StringQuery {
                            operator: Some(string_query::Operator::Equals(event_id.clone())),
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

            *registrations.write() = response
                .into_inner()
                .registrations
                .into_iter()
                .map(|r| r.into())
                .collect();

            true
        }
    });

    if !registrations_loaded.value().unwrap_or(&false) {
        return None;
    }

    let schema = match schema.value().and_then(|s| s.as_ref()) {
        Some(s) => s,
        None => return None,
    };

    let show_modal = use_state(cx, || false);

    cx.render(rsx! {
        GenericPage {
            title: "View Registrations".to_owned(),
            Button {
                flavor: ButtonFlavor::Info,
                onclick: |_| {
                    show_modal.set(true);
                },
                "Add Registration",
            }
            Table {
                is_striped: true,
                is_fullwidth: true,
                thead {
                    tr {
                        schema.items.iter().map(|item| {
                            rsx! {
                                th {
                                    key: "{item.id}",
                                    "{item.name}"
                                }
                            }
                        })
                    }
                }
                tbody {
                    registrations.read().iter().map(|registration| {
                        rsx! {
                            tr {
                                key: "{registration.id}",
                                schema.items.iter().map(|item| {
                                    rsx! {
                                        td {
                                            key: "{item.id}",
                                            registration.items.get(&item.id).map(|v| v.as_str()).unwrap_or_default()
                                        }
                                    }
                                })
                            }
                        }
                    })
                }
            }

            if **show_modal {
                rsx!{
                    RegistrationModal {
                        schema: schema,
                        do_submit: |registration| {
                            cx.spawn({
                                to_owned!(grpc_client, event_id, toaster, registrations, registration, show_modal);
                                async move {
                                    let result = grpc_client
                                        .registration
                                        .upsert_registrations(UpsertRegistrationsRequest{
                                            registrations: vec![to_proto_registration(registration, event_id)]
                                        })
                                        .await;

                                    let response = match result {
                                        Ok(rsp) => rsp,
                                        Err(e) => {
                                            toaster.write().new_error(e.to_string());
                                            return;
                                        }
                                    };

                                    registrations.write().extend(response.into_inner().registrations.into_iter().map(|r| r.into()));

                                    show_modal.set(false);
                                }
                            })
                        },
                        do_close: || {
                            show_modal.set(false);
                        }
                    }
                }
            }
        }
    })
}

#[derive(Clone, Debug, PartialEq)]
struct SelectOption {
    options: Vec<String>,
    selected: usize,
    other: String,
}

impl From<SelectOption> for String {
    fn from(select: SelectOption) -> Self {
        if select.selected == select.options.len() {
            select.other
        } else {
            select.options[select.selected].clone()
        }
    }
}

impl SelectOption {
    fn new(options: Vec<String>, selected: usize) -> Self {
        let options = options
            .into_iter()
            .chain(std::iter::once("Other".to_owned()))
            .collect();

        Self {
            options,
            selected,
            other: "".to_owned(),
        }
    }
}

#[derive(Clone, PartialEq)]
struct MultiSelectOption {
    options: Vec<String>,
    selected: BTreeSet<usize>,
    is_other: bool,
    other: String,
}

impl From<MultiSelectOption> for String {
    fn from(select: MultiSelectOption) -> Self {
        if select.is_other {
            select.other
        } else {
            itertools::Itertools::intersperse(
                select.selected.iter().map(|i| select.options[*i].clone()),
                ",".to_owned(),
            )
            .collect()
        }
    }
}

impl MultiSelectOption {
    fn new(options: Vec<String>, selected: BTreeSet<usize>) -> Self {
        Self {
            options,
            selected,
            is_other: false,
            other: "".to_owned(),
        }
    }
}

#[derive(Clone)]
enum FormRegistrationItemValue {
    Text(String),
    Checkbox(SelectOption),
    Select(SelectOption),
    MultiSelect(MultiSelectOption),
}

impl From<FormRegistrationItemValue> for String {
    fn from(value: FormRegistrationItemValue) -> Self {
        match value {
            FormRegistrationItemValue::Text(v) => v,
            FormRegistrationItemValue::Checkbox(v) => v.into(),
            FormRegistrationItemValue::Select(v) => v.into(),
            FormRegistrationItemValue::MultiSelect(v) => v.into(),
        }
    }
}

impl FormRegistrationItemValue {
    fn try_as_checkbox_mut(&mut self) -> Option<&mut SelectOption> {
        match self {
            FormRegistrationItemValue::Checkbox(v) => Some(v),
            _ => None,
        }
    }

    fn try_as_select_mut(&mut self) -> Option<&mut SelectOption> {
        match self {
            FormRegistrationItemValue::Select(v) => Some(v),
            _ => None,
        }
    }

    fn try_as_multi_select_mut(&mut self) -> Option<&mut MultiSelectOption> {
        match self {
            FormRegistrationItemValue::MultiSelect(v) => Some(v),
            _ => None,
        }
    }
}

struct FormRegistrationItem {
    schema_item_id: String,
    name: String,
    value: FormRegistrationItemValue,
}

#[component]
fn RegistrationModal<'a, DoSubmit: Fn(TableRegistration) -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    schema: &'a RegistrationSchema,
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let form = use_ref(cx, || {
        schema
            .items
            .iter()
            .map(|item| {
                let value = match item
                    .r#type
                    .as_ref()
                    .and_then(|t| t.r#type.as_ref())
                    .unwrap()
                {
                    registration_schema_item_type::Type::Text(_) => {
                        FormRegistrationItemValue::Text("".to_owned())
                    }

                    registration_schema_item_type::Type::Checkbox(checkbox) => {
                        let selected = if checkbox.default { 1 } else { 0 };

                        FormRegistrationItemValue::Checkbox(SelectOption::new(
                            vec!["No".to_owned(), "Yes".to_owned()],
                            selected,
                        ))
                    }

                    registration_schema_item_type::Type::Select(select) => {
                        let selected = select.default as usize;

                        FormRegistrationItemValue::Select(SelectOption::new(
                            select.options.iter().map(|o| o.name.clone()).collect(),
                            selected,
                        ))
                    }

                    registration_schema_item_type::Type::MultiSelect(select) => {
                        let selected = select
                            .defaults
                            .iter()
                            .cloned()
                            .map(|d| d as usize)
                            .collect();

                        FormRegistrationItemValue::MultiSelect(MultiSelectOption::new(
                            select.options.iter().map(|o| o.name.clone()).collect(),
                            selected,
                        ))
                    }
                };

                FormRegistrationItem {
                    schema_item_id: item.id.clone(),
                    name: item.name.clone(),
                    value,
                }
            })
            .collect::<Vec<_>>()
    });
    let submitted = use_state(cx, || false);

    cx.render(rsx! {
        Modal {
            title: "Add Registration",
            do_close: do_close,
            do_submit: || {
                submitted.set(true);
                let items = form.read().iter().map(|item| {
                    (item.schema_item_id.clone(), item.value.clone().into())
                }).collect();
                let r = TableRegistration {
                    id: "".to_owned(),
                    items,
                };
                do_submit(r)
            },
            disable_submit: **submitted,

            form {
                form.read().iter().enumerate().map(|(idx, item)| {
                    rsx! {
                        Field {
                            key: "{item.schema_item_id}",
                            label: "{item.name}",
                            match item.value.clone() {
                                FormRegistrationItemValue::Text(value) => {
                                    rsx! {
                                        TextRegistrationForm {
                                            value: value,
                                            do_input: move |v| {
                                                form.write()[idx].value = FormRegistrationItemValue::Text(v);
                                            },
                                        }
                                    }
                                }
                                FormRegistrationItemValue::Checkbox(select_option) => {
                                    rsx! {
                                        SelectRegistrationForm {
                                            select_option: select_option,
                                            do_select_input: move |v| {
                                                form.write()[idx].value.try_as_checkbox_mut().unwrap().selected = v;
                                            },
                                            do_other_input: move |v| {
                                                form.write()[idx].value.try_as_checkbox_mut().unwrap().other = v;
                                            },
                                        }
                                    }

                                }
                                FormRegistrationItemValue::Select(select_option) => {
                                    rsx! {
                                        SelectRegistrationForm {
                                            select_option: select_option.clone(),
                                            do_select_input: move |v| {
                                                form.write()[idx].value.try_as_select_mut().unwrap().selected = v;
                                            },
                                            do_other_input: move |v| {
                                                form.write()[idx].value.try_as_select_mut().unwrap().other = v;
                                            },
                                        }
                                    }

                                }
                                FormRegistrationItemValue::MultiSelect(multi_select_option) => {
                                    rsx! {
                                        MultiSelectRegistrationForm {
                                            select_option: multi_select_option.clone(),
                                            do_select_input: move |option_idx, ctrl| {
                                                if ctrl {
                                                    if multi_select_option.selected.contains(&option_idx) {
                                                        form.write()[idx].value.try_as_multi_select_mut().unwrap().selected.remove(&option_idx);
                                                    } else {
                                                        form.write()[idx].value.try_as_multi_select_mut().unwrap().selected.insert(option_idx);
                                                    }
                                                } else {
                                                    form.with_mut(|form| {
                                                        let selected = &mut form[idx].value.try_as_multi_select_mut().unwrap().selected;
                                                        selected.clear();
                                                        selected.insert(option_idx);
                                                    })
                                                }
                                            },
                                            do_other_input: move |v| {
                                                form.write()[idx].value.try_as_multi_select_mut().unwrap().other = v;
                                            },
                                            do_is_other_input: move || {
                                                form.write()[idx].value.try_as_multi_select_mut().unwrap().is_other = !multi_select_option.is_other;
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                })
            }
        }
    })
}

#[component]
fn TextRegistrationForm<DoInput: Fn(String) -> ()>(
    cx: Scope,
    value: String,
    do_input: DoInput,
) -> Element {
    cx.render(rsx! {
        TextInput {
            oninput: move |evt: FormEvent| {
                do_input(evt.value.clone());
            },
            value: TextInputType::Text(value.clone()),
        }
    })
}

#[component]
fn SelectRegistrationForm<DoSelectInput: Fn(usize) -> (), DoOtherInput: Fn(String) -> ()>(
    cx: Scope,
    select_option: SelectOption,
    do_select_input: DoSelectInput,
    do_other_input: DoOtherInput,
) -> Element {
    cx.render(rsx! {
        div {
            class: "box",
            Field {
                label: "Value",
                SelectInput {
                    options: select_option.options.clone(),
                    onchange: move |evt: FormEvent| {
                        do_select_input(evt.value.parse().unwrap());
                    },
                    value: select_option.selected,
                }
            }
            if select_option.selected == (select_option.options.len() - 1) {
                rsx!{
                    Field {
                        label: "Other",
                        TextInput {
                            oninput: move |evt: FormEvent| {
                                do_other_input(evt.value.clone());
                            },
                            value: TextInputType::Text(select_option.other.clone()),
                        }
                    }
                }
            }
        }
    })
}

#[component]
fn MultiSelectRegistrationForm<
    DoIsOtherInput: Fn() -> (),
    DoSelectInput: Fn(usize, bool) -> (),
    DoOtherInput: Fn(String) -> (),
>(
    cx: Scope,
    select_option: MultiSelectOption,
    do_is_other_input: DoIsOtherInput,
    do_select_input: DoSelectInput,
    do_other_input: DoOtherInput,
) -> Element {
    cx.render(rsx! {
        div {
            class: "box",
            CheckInput{
                style: CheckStyle::Checkbox,
                label: "Use \"Other\"",
                value: select_option.is_other,
                onclick: move |_| {
                    do_is_other_input();
                }
            }
            if select_option.is_other {
                rsx!{
                    Field {
                        label: "Other",
                        TextInput {
                            oninput: move |evt: FormEvent| {
                                do_other_input(evt.value.clone());
                            },
                            value: TextInputType::Text(select_option.other.clone()),
                        }
                    }
                }
            } else {
                rsx!{
                    Field {
                        label: "Value",
                        MultiSelectInput {
                            options: select_option.options.clone(),
                            do_select: move |idx, evt| {
                                do_select_input(idx, evt.modifiers().ctrl());
                            },
                            value: select_option.selected.clone().into_iter().collect(),
                        }
                    }
                }
            }
        }
    })
}

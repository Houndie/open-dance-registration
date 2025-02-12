use crate::{
    hooks::{handle_error::use_handle_error, toasts::use_toasts},
    server_functions::{
        authentication::claims, event::query as query_events,
        organization::query as query_organizations, registration::query as query_registrations,
        registration::upsert as upsert_registrations,
        registration_schema::query as query_registration_schemas, ProtoWrapper,
    },
    view::{
        app::{Error, Routes},
        components::{
            form::{
                Button, ButtonFlavor, CheckInput, CheckStyle, Field, MultiSelectInput, SelectInput,
                TextInput, TextInputType,
            },
            modal::Modal,
            page::Page as GenericPage,
            table::Table,
        },
        pages::event::{Menu, MenuItem},
    },
};
use common::proto::{
    self, event_query, organization_query, registration_query, registration_schema_item_type,
    registration_schema_query, string_query, ClaimsRequest, EventQuery, Organization,
    OrganizationQuery, QueryEventsRequest, QueryOrganizationsRequest,
    QueryRegistrationSchemasRequest, QueryRegistrationsRequest, Registration, RegistrationItem,
    RegistrationQuery, RegistrationSchema, RegistrationSchemaQuery, StringQuery,
    UpsertRegistrationsRequest,
};
use dioxus::prelude::*;
use std::collections::{BTreeSet, HashMap, HashSet};

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

        let schema_future = query_registration_schemas(QueryRegistrationSchemasRequest {
            query: Some(RegistrationSchemaQuery {
                query: Some(registration_schema_query::Query::EventId(StringQuery {
                    operator: Some(string_query::Operator::Equals(id())),
                })),
            }),
        });

        let registrations_future = query_registrations(QueryRegistrationsRequest {
            query: Some(RegistrationQuery {
                query: Some(registration_query::Query::EventId(StringQuery {
                    operator: Some(string_query::Operator::Equals(id())),
                })),
            }),
        });

        let claims = claims_future
            .await
            .map_err(Error::from_server_fn_error)?
            .claims
            .ok_or(Error::Unauthenticated)?;

        let mut events_response = events_future.await.map_err(Error::from_server_fn_error)?;
        let event = events_response.events.pop().ok_or(Error::NotFound)?;

        let organization_future = query_organizations(QueryOrganizationsRequest {
            query: Some(OrganizationQuery {
                query: Some(organization_query::Query::Id(StringQuery {
                    operator: Some(string_query::Operator::Equals(
                        event.organization_id.clone(),
                    )),
                })),
            }),
        });

        let mut schema_response = schema_future.await.map_err(Error::from_server_fn_error)?;
        let schema = schema_response
            .registration_schemas
            .pop()
            .unwrap_or_else(move || RegistrationSchema {
                event_id: id(),
                ..Default::default()
            });

        let registrations_response = registrations_future
            .await
            .map_err(Error::from_server_fn_error)?;

        let mut organization_response = organization_future
            .await
            .map_err(Error::from_server_fn_error)?;
        let organization = organization_response
            .organizations
            .pop()
            .ok_or(Error::Misc("organization not found".to_owned()))?;

        Ok((
            ProtoWrapper(claims),
            ProtoWrapper(organization),
            ProtoWrapper(event),
            ProtoWrapper(schema),
            ProtoWrapper(registrations_response),
        ))
    })?;

    use_handle_error(
        results.suspend()?,
        |(
            ProtoWrapper(claims),
            ProtoWrapper(organization),
            ProtoWrapper(event),
            ProtoWrapper(schema),
            ProtoWrapper(registrations_response),
        )| {
            rsx! {
                GenericPage {
                    title: "View Registrations".to_owned(),
                    breadcrumb: vec![
                        ("Home".to_owned(), Some(Routes::LandingPage)),
                        (organization.name.clone(), Some(Routes::OrganizationPage { org_id: organization.id.clone() })),
                        (event.name.clone(), Some(Routes::EventPage{ id: event.id.clone() })),
                        ("Registrations".to_owned(), None),
                    ],
                    menu: rsx!{
                        Menu {
                            event_name: event.name.clone(),
                            event_id: event.id.clone(),
                            highlight: MenuItem::Registrations,
                        }
                    },
                    claims: claims,
                    PageBody{
                        org: organization,
                        event: event,
                        schema: schema,
                        registrations: registrations_response.registrations,
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
    schema: ReadOnlySignal<RegistrationSchema>,
    registrations: ReadOnlySignal<Vec<Registration>>,
) -> Element {
    let mut toaster = use_toasts();

    let mut registrations = use_signal(move || {
        registrations
            .read()
            .iter()
            .map(|r| -> TableRegistration { r.clone().into() })
            .collect::<Vec<_>>()
    });

    let mut show_modal = use_signal(|| None);
    let registration_modal = show_modal.read().as_ref().map(move |modal_registration: &TableRegistration| {
            rsx!{
                RegistrationModal {
                    schema: schema,
                    onsubmit: move |registration| {
                        spawn(async move {
                            let result = upsert_registrations(UpsertRegistrationsRequest{
                                    registrations: vec![to_proto_registration(registration, event().id.clone())]
                                })
                                .await;

                            let mut response = match result {
                                Ok(rsp) => rsp,
                                Err(e) => {
                                    toaster.write().new_error(e.to_string());
                                    return;
                                }
                            };

                            let response_registration = match response.registrations.pop() {
                                Some(r) => r,
                                None => {
                                    toaster.write().new_error("No registration returned".to_owned());
                                    return;
                                }
                            };

                            let position = registrations.read().iter().position(|r| r.id == response_registration.id);

                            match position {
                                Some(idx) => {
                                    registrations.write()[idx] = response_registration.into();
                                }
                                None => {
                                    registrations.write().push(response_registration.into());
                                }
                            }

                            show_modal.set(None);
                        });
                    },
                    onclose: move |_| {
                        show_modal.set(None);
                    },
                    registration: modal_registration.clone(),
                }
            }
        });

    rsx! {
        Button {
            flavor: ButtonFlavor::Info,
            onclick: move |_| {
                show_modal.set(Some(TableRegistration::default()));
            },
            "Add Registration",
        }
        Table {
            is_striped: true,
            is_fullwidth: true,
            thead {
                tr {
                    th{}
                    { schema.read().items.iter().map(|item| {
                        rsx! {
                            th {
                                key: "{item.id}",
                                "{item.name}"
                            }
                        }
                    })}
                }
            }
            tbody {
                {registrations.read().iter().map(|registration| {
                    let button_registration = registration.clone();
                    rsx! {
                        tr {
                            key: "{registration.id}",
                            td {
                                Button {
                                    flavor: ButtonFlavor::Info,
                                    onclick: move |_| {
                                        show_modal.set(Some(button_registration.clone()));
                                    },
                                    "Edit"
                                }
                            }
                            { schema.read().items.iter().map(|item| {
                                rsx! {
                                    td {
                                        key: "{item.id}",
                                        {registration.items.get(&item.id).map(|v| v.as_str()).unwrap_or_default()}
                                    }
                                }
                            })}
                        }
                    }
                })}
            }
        }
        {registration_modal}
    }
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

    fn from_existing(options: Vec<String>, existing: String) -> Self {
        let selected = options
            .iter()
            .position(|o| o == &existing)
            .unwrap_or(options.len());

        let other = if selected == options.len() {
            existing
        } else {
            "".to_owned()
        };

        let options = options
            .into_iter()
            .chain(std::iter::once("Other".to_owned()))
            .collect::<Vec<_>>();

        Self {
            options,
            selected,
            other,
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

    fn from_existing(options: Vec<String>, existing: String) -> Self {
        let selected = existing
            .split(',')
            .map(|s| options.iter().position(|o| o == s))
            .collect::<Vec<_>>();

        if selected.iter().any(|s| s.is_none()) {
            let other = existing;
            Self {
                options,
                selected: BTreeSet::new(),
                is_other: true,
                other,
            }
        } else {
            Self {
                options,
                selected: selected.into_iter().map(|s| s.unwrap()).collect(),
                is_other: false,
                other: "".to_owned(),
            }
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
fn RegistrationModal(
    schema: ReadOnlySignal<RegistrationSchema>,
    registration: ReadOnlySignal<TableRegistration>,
    onsubmit: EventHandler<TableRegistration>,
    onclose: EventHandler<()>,
) -> Element {
    let mut form = use_signal(move || {
        schema
            .read()
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
                        FormRegistrationItemValue::Text(
                            registration
                                .read()
                                .items
                                .get(&item.id)
                                .cloned()
                                .unwrap_or_default(),
                        )
                    }

                    registration_schema_item_type::Type::Checkbox(checkbox) => {
                        let options = vec!["No".to_owned(), "Yes".to_owned()];
                        let select_option = match registration.read().items.get(&item.id) {
                            Some(registration_item) => {
                                SelectOption::from_existing(options, registration_item.clone())
                            }
                            None => {
                                let selected = if checkbox.default { 1 } else { 0 };
                                SelectOption::new(options, selected)
                            }
                        };

                        FormRegistrationItemValue::Checkbox(select_option)
                    }

                    registration_schema_item_type::Type::Select(select) => {
                        let options = select.options.iter().map(|o| o.name.clone()).collect();
                        let select_option = match registration.read().items.get(&item.id) {
                            Some(registration_item) => {
                                SelectOption::from_existing(options, registration_item.clone())
                            }
                            None => {
                                let selected = select.default as usize;
                                SelectOption::new(options, selected)
                            }
                        };

                        FormRegistrationItemValue::Select(select_option)
                    }

                    registration_schema_item_type::Type::MultiSelect(select) => {
                        let options = select.options.iter().map(|o| o.name.clone()).collect();
                        let select_option = match registration.read().items.get(&item.id) {
                            Some(registration_item) => {
                                MultiSelectOption::from_existing(options, registration_item.clone())
                            }
                            None => {
                                let selected = select
                                    .defaults
                                    .iter()
                                    .cloned()
                                    .map(|d| d as usize)
                                    .collect();
                                MultiSelectOption::new(options, selected)
                            }
                        };

                        FormRegistrationItemValue::MultiSelect(select_option)
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
    let mut submitted = use_signal(|| false);

    let (title, success_text) = if registration.read().id.is_empty() {
        ("Add Registration", "Create")
    } else {
        ("Edit Registration", "Update")
    };

    rsx! {
        Modal {
            title: "{title}",
            onclose: onclose,
            onsubmit: move |_| {
                submitted.set(true);
                let items = form.read().iter().map(|item| {
                    (item.schema_item_id.clone(), item.value.clone().into())
                }).collect();
                let r = TableRegistration {
                    id: registration.read().id.clone(),
                    items,
                };
                onsubmit.call(r)
            },
            disable_submit: *submitted.read(),
            success_text: "{success_text}",

            form {
                { form.read().iter().enumerate().map(|(idx, item)| {
                    rsx! {
                        Field {
                            key: "{item.schema_item_id}",
                            label: "{item.name}",
                            match item.value.clone() {
                                FormRegistrationItemValue::Text(value) => {
                                    rsx! {
                                        TextRegistrationForm {
                                            value: value,
                                            oninput: move |v| {
                                                form.write()[idx].value = FormRegistrationItemValue::Text(v);
                                            },
                                        }
                                    }
                                }
                                FormRegistrationItemValue::Checkbox(select_option) => {
                                    rsx! {
                                        SelectRegistrationForm {
                                            select_option: select_option,
                                            onselectinput: move |v| {
                                                form.write()[idx].value.try_as_checkbox_mut().unwrap().selected = v;
                                            },
                                            onotherinput: move |v| {
                                                form.write()[idx].value.try_as_checkbox_mut().unwrap().other = v;
                                            },
                                        }
                                    }

                                }
                                FormRegistrationItemValue::Select(select_option) => {
                                    rsx! {
                                        SelectRegistrationForm {
                                            select_option: select_option.clone(),
                                            onselectinput: move |v| {
                                                form.write()[idx].value.try_as_select_mut().unwrap().selected = v;
                                            },
                                            onotherinput: move |v| {
                                                form.write()[idx].value.try_as_select_mut().unwrap().other = v;
                                            },
                                        }
                                    }

                                }
                                FormRegistrationItemValue::MultiSelect(multi_select_option) => {
                                    rsx! {
                                        MultiSelectRegistrationForm {
                                            select_option: multi_select_option.clone(),
                                            onselectinput: move |(option_idx, ctrl)| {
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
                                            onotherinput: move |v| {
                                                form.write()[idx].value.try_as_multi_select_mut().unwrap().other = v;
                                            },
                                            onisotherinput: move |_| {
                                                form.write()[idx].value.try_as_multi_select_mut().unwrap().is_other = !multi_select_option.is_other;
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    }
                })}
            }
        }
    }
}

#[component]
fn TextRegistrationForm(value: ReadOnlySignal<String>, oninput: EventHandler<String>) -> Element {
    rsx! {
        TextInput {
            oninput: move |evt: FormEvent| {
                oninput.call(evt.value());
            },
            value: TextInputType::Text(value.read().clone()),
        }
    }
}

#[component]
fn SelectRegistrationForm(
    select_option: SelectOption,
    onselectinput: EventHandler<usize>,
    onotherinput: EventHandler<String>,
) -> Element {
    let other = if select_option.selected == (select_option.options.len() - 1) {
        rsx! {
            Field {
                label: "Other",
                TextInput {
                    oninput: move |evt: FormEvent| {
                        onotherinput.call(evt.value());
                    },
                    value: TextInputType::Text(select_option.other.clone()),
                }
            }
        }
    } else {
        rsx! {}
    };

    rsx! {
        div {
            class: "box",
            Field {
                label: "Value",
                SelectInput {
                    options: select_option.options.clone(),
                    onchange: move |evt: FormEvent| {
                        onselectinput.call(evt.value().parse().unwrap());
                    },
                    value: select_option.selected,
                }
            }
            { other }
        }
    }
}

#[component]
fn MultiSelectRegistrationForm(
    select_option: MultiSelectOption,
    onisotherinput: EventHandler<()>,
    onselectinput: EventHandler<(usize, bool)>,
    onotherinput: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            class: "box",
            CheckInput{
                style: CheckStyle::Checkbox,
                label: "Use \"Other\"".to_owned(),
                value: select_option.is_other,
                onclick: move |_| {
                    onisotherinput.call(());
                }
            }
            { if select_option.is_other {
                rsx!{
                    Field {
                        label: "Other",
                        TextInput {
                            oninput: move |evt: FormEvent| {
                                onotherinput.call(evt.value());
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
                            onselect: move |(idx, evt): (usize, MouseEvent)| {
                                onselectinput.call((idx, evt.modifiers().ctrl()));
                            },
                            value: select_option.selected.clone().into_iter().collect::<HashSet<usize>>(),
                        }
                    }
                }
            }}
        }
    }
}

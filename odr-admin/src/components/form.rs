use dioxus::prelude::*;

#[derive(Clone)]
pub enum TextInputType {
    Text(String),
    Number(i32),
}

#[component]
pub fn TextInput<'a>(
    cx: Scope,
    oninput: EventHandler<'a, FormEvent>,
    value: TextInputType,
    label: &'a str,
    input_id: Option<&'a str>,
) -> Element<'a> {
    let input_id = match input_id {
        Some(input_id) => String::from(*input_id),
        None => format!("form-{}-id", label),
    };

    let value_str = match value {
        TextInputType::Text(text) => text.clone(),
        TextInputType::Number(number) => format!("{}", number),
    };

    let typ = match value {
        TextInputType::Text(_) => "text",
        TextInputType::Number(_) => "number",
    };

    cx.render(rsx!(
        div {
            class: "field is-horizontal",
            div {
                class: "field-label is-normal",
                label {
                    "for": "{input_id}",
                    class: "label",
                    "{label}"
                }
            }
            div {
                class: "field-body",
                div {
                    class: "field",
                    div {
                        class: "control",
                        input {
                            id: "{input_id}",
                            class: "input",
                            value: "{value_str}",
                            "type": typ,
                            oninput: move |evt| oninput.call(evt),
                        }
                    }
                }
            }
        }
    ))
}

#[component]
pub fn SelectInput<'a>(
    cx: Scope,
    onchange: EventHandler<'a, FormEvent>,
    options: Vec<String>,
    value: usize,
    label: &'a str,
    input_id: Option<&'a str>,
) -> Element<'a> {
    let input_id = match input_id {
        Some(input_id) => String::from(*input_id),
        None => format!("form-{}-id", label),
    };

    cx.render(rsx!(
        div {
            class: "mb-3",
            label {
                "for": "{input_id}",
                class: "form-label",
                "{label}"
            }
            select {
                id: "{input_id}",
                class: "form-select",
                onchange: move |evt| onchange.call(evt),
                value: "{value}",
                options.iter().enumerate().map(|(idx, v)| rsx!(
                    option {
                        key: "{idx}",
                        value: "{idx}",
                        "{v}"
                    }
                ))
            }
        }
    ))
}

#[component]
pub fn CheckInput<'a>(
    cx: Scope,
    onclick: EventHandler<'a, MouseEvent>,
    value: bool,
    label: &'a str,
    input_id: Option<&'a str>,
) -> Element<'a> {
    let input_id = match input_id {
        Some(input_id) => String::from(*input_id),
        None => format!("form-{}-id", label),
    };
    log::info!("{}", value);

    cx.render(rsx!(
        div {
            class: "mb-3",
            div {
                class: "form-check",
                input {
                    id: "{input_id}",
                    r#type: "checkbox",
                    checked: *value,
                    prevent_default: "onclick",
                    onclick: move |evt| onclick.call(evt),
                }
                label {
                    "for": "{input_id}",
                    class: "form-label",
                    "{label}"
                }
            }
        }
    ))
}

pub enum ButtonFlavor {
    Info,
    Success,
}

#[component]
pub fn Button<'a>(
    cx: Scope,
    onclick: EventHandler<'a, MouseEvent>,
    flavor: Option<ButtonFlavor>,
    disabled: Option<bool>,
    children: Element<'a>,
) -> Element {
    let mut class = "button".to_owned();

    match flavor {
        None => {}
        Some(ButtonFlavor::Info) => class.push_str(" is-info"),
        Some(ButtonFlavor::Success) => class.push_str(" is-success"),
    };

    cx.render(rsx!(
        button {
            class: "{class}",
            disabled: *disabled,
            onclick: move |evt| onclick.call(evt),
            &children
        }
    ))
}

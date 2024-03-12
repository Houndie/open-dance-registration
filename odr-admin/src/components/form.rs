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
    onblur: Option<EventHandler<'a, FocusEvent>>,
    invalid: Option<Option<String>>,
    value: TextInputType,
    is_expanded: Option<bool>,
) -> Element<'a> {
    let value_str = match value {
        TextInputType::Text(text) => text.clone(),
        TextInputType::Number(number) => format!("{}", number),
    };

    let typ = match value {
        TextInputType::Text(_) => "text",
        TextInputType::Number(_) => "number",
    };

    let class = "field".to_owned();
    let class = if matches!(*is_expanded, Some(true)) {
        format!("{} is-expanded", class)
    } else {
        class
    };

    let invalid = invalid.as_ref().map(|o| o.as_ref()).flatten();

    let input_class = "input".to_owned();
    let input_class = if invalid.is_some() {
        format!("{} is-danger", input_class)
    } else {
        input_class
    };

    cx.render(rsx!(
        div {
            class: "{class}",
            div {
                class: "control",
                input {
                    class: "{input_class}",
                    value: "{value_str}",
                    "type": typ,
                    oninput: move |evt| oninput.call(evt),
                    onblur: move |evt| match onblur {
                        Some(onblur) => onblur.call(evt),
                        None => (),
                    },
                }
            }
            if let Some(invalid) = invalid {
                rsx!{
                    p {
                        class: "help is-danger",
                        "{invalid}"
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
) -> Element<'a> {
    cx.render(rsx!(
        div {
            class: "field",
            div {
                class: "control",
                select {
                    class: "select",
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
        }
    ))
}

pub enum CheckStyle {
    Checkbox,
    Radio,
}

#[component]
pub fn CheckInput<'a>(
    cx: Scope,
    style: CheckStyle,
    label: Option<&'a str>,
    onclick: EventHandler<'a, MouseEvent>,
    value: bool,
) -> Element<'a> {
    let style_str = match style {
        CheckStyle::Checkbox => "checkbox",
        CheckStyle::Radio => "radio",
    };

    let input = rsx!(input {
        class: "{style_str}",
        r#type: "{style_str}",
        checked: *value,
        onclick: move |evt| onclick.call(evt),
    });

    cx.render(rsx!(
        div {
            class: "field",
            div {
                class: "control",
                match label {
                    None => input,
                    Some(label) => rsx!(
                        label {
                            class: "checkbox",
                            input
                            "{label}",
                        }
                    ),
                }
            }
        }
    ))
}

pub enum ButtonFlavor {
    Info,
    Success,
    Danger,
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
        Some(ButtonFlavor::Danger) => class.push_str(" is-danger"),
    };

    cx.render(rsx!(
        button {
            class: "{class}",
            disabled: *disabled,
            "type": "button",
            onclick: move |evt| onclick.call(evt),
            &children
        }
    ))
}

#[component]
pub fn Field<'a>(
    cx: Scope,
    onmounted: Option<EventHandler<'a, MountedEvent>>,
    ondragover: Option<EventHandler<'a, DragEvent>>,
    label: &'a str,
    children: Element<'a>,
) -> Element {
    cx.render(rsx!(
        div {
            onmounted: move |evt| match onmounted {
                Some(onmounted) => onmounted.call(evt),
                None => (),
            },
            ondragover: move |evt| match ondragover {
                Some(ondragover) => ondragover.call(evt),
                None => (),
            },
            class: "field is-horizontal",
            div {
                class: "field-label is-normal",
                label {
                    class: "label",
                    "{label}"
                }
            }
            div {
                class: "field-body",
                &children
            }
        }
    ))
}

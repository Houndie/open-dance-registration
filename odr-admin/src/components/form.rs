use std::collections::HashSet;

use dioxus::prelude::*;

#[derive(Clone)]
pub enum TextInputType {
    Text(String),
    Password(String),
    Number(i32),
}

#[component]
pub fn TextInput(
    oninput: EventHandler<FormEvent>,
    onblur: Option<EventHandler<FocusEvent>>,
    invalid: Option<Option<String>>,
    value: ReadOnlySignal<TextInputType>,
    is_expanded: Option<bool>,
) -> Element {
    let value_str = match &*value.read() {
        TextInputType::Text(text) => text.clone(),
        TextInputType::Password(text) => text.clone(),
        TextInputType::Number(number) => format!("{}", number),
    };

    let typ = match &*value.read() {
        TextInputType::Text(_) => "text",
        TextInputType::Number(_) => "number",
        TextInputType::Password(_) => "password",
    };

    let class = "field".to_owned();
    let class = if matches!(is_expanded, Some(true)) {
        format!("{} is-expanded", class)
    } else {
        class
    };

    let invalid = invalid.flatten().map(|invalid| {
        rsx! {
            p {
                class: "help is-danger",
                "{invalid}"
            }
        }
    });

    let input_class = "input".to_owned();
    let input_class = if invalid.is_some() {
        format!("{} is-danger", input_class)
    } else {
        input_class
    };

    rsx! {
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
            { invalid }
        }
    }
}

#[component]
pub fn SelectInput(
    onchange: EventHandler<FormEvent>,
    options: ReadOnlySignal<Vec<String>>,
    value: usize,
) -> Element {
    rsx! {
        div {
            class: "select",
            select {
                onchange: move |evt| onchange.call(evt),
                value: "{value}",
                { options.iter().enumerate().map(|(idx, v)| {
                    let selected = value == idx;
                    rsx!(
                        option {
                            selected: selected,
                            key: "{idx}",
                            value: "{idx}",
                            "{v}"
                        }
                    )
                })}
            }
        }
    }
}

#[component]
pub fn MultiSelectInput(
    onselect: EventHandler<(usize, MouseEvent)>,
    options: ReadOnlySignal<Vec<String>>,
    value: ReadOnlySignal<HashSet<usize>>,
) -> Element {
    rsx! {
        div {
            class: "select is-multiple",
            select {
                multiple: true,
                { options.iter().enumerate().map(|(idx, v)| {
                    let selected = value.read().contains(&idx);
                    rsx!(
                        option {
                            key: "{idx}",
                            value: "{idx}",
                            selected: selected,
                            onclick: move |evt| onselect.call((idx, evt)),
                            "{v}"
                        }
                    )
                }) }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum CheckStyle {
    Checkbox,
    Radio,
}

#[component]
pub fn CheckInput(
    style: CheckStyle,
    label: Option<String>,
    onclick: EventHandler<MouseEvent>,
    value: bool,
) -> Element {
    let style_str = match style {
        CheckStyle::Checkbox => "checkbox",
        CheckStyle::Radio => "radio",
    };

    let input = rsx!(input {
        class: "{style_str}",
        r#type: "{style_str}",
        checked: value,
        onclick: move |evt| onclick.call(evt),
    });

    rsx! {
        div {
            class: "field",
            div {
                class: "control",
                { match label {
                    None => input,
                    Some(label) => rsx!(
                        label {
                            class: "checkbox",
                            { input }
                            "{label}",
                        }
                    ),
                } }
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ButtonFlavor {
    Info,
    Success,
    Danger,
}

#[component]
pub fn Button(
    onclick: EventHandler<MouseEvent>,
    flavor: Option<ButtonFlavor>,
    disabled: Option<bool>,
    children: Element,
) -> Element {
    let mut class = "button".to_owned();

    match flavor {
        None => {}
        Some(ButtonFlavor::Info) => class.push_str(" is-info"),
        Some(ButtonFlavor::Success) => class.push_str(" is-success"),
        Some(ButtonFlavor::Danger) => class.push_str(" is-danger"),
    };

    rsx! {
         button {
             class: "{class}",
             disabled: disabled,
             "type": "button",
             onclick: move |evt| onclick.call(evt),
             { children }
         }
    }
}

#[component]
pub fn Field(
    onmounted: Option<EventHandler<MountedEvent>>,
    ondragover: Option<EventHandler<DragEvent>>,
    label: ReadOnlySignal<String>,
    children: Element,
) -> Element {
    rsx! {
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
                { children }
            }
        }
    }
}

use dioxus::prelude::*;

#[component]
pub fn TextInput<'a>(
    cx: Scope,
    oninput: EventHandler<'a, FormEvent>,
    value: String,
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
            input {
                id: "{input_id}",
                class: "form-control",
                value: "{value}",
                oninput: move |evt| oninput.call(evt),
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

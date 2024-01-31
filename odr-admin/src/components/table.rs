use dioxus::prelude::*;

#[component]
pub fn Table<'a>(
    cx: Scope,
    children: Element<'a>,
    is_striped: Option<bool>,
    is_fullwidth: Option<bool>,
) -> Element {
    let mut class = "table".to_owned();

    if let Some(is_striped) = is_striped {
        if *is_striped {
            class.push_str(" is-striped");
        }
    };

    if let Some(is_fullwidth) = is_fullwidth {
        if *is_fullwidth {
            class.push_str(" is-fullwidth");
        }
    };

    cx.render(rsx! {
        table {
            class: "{class}",
            &children
        }
    })
}

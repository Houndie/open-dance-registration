use crate::{
    components::{
        form::{SelectInput, TextInput},
        modal::Modal,
        page::Page as GenericPage,
    },
    hooks::toasts::use_toasts,
};
use common::proto::{
    multi_select_type, select_type, CheckboxType, RegistrationSchemaItem, SelectOption, TextType,
};
use dioxus::prelude::*;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
    log::info!("{}", id); // temporarily silencing warning
    let show_schema_item_modal = use_state(cx, || false);
    cx.render(rsx! {
        GenericPage {
            title: "Modify Registration Schema".to_owned(),
            button {
                class: "btn btn-primary",
                onclick: |_| show_schema_item_modal.set(true),
                "Add Field"
            }
        }
        if **show_schema_item_modal {
            rsx!(NewSchemaItemModal{
                do_submit: |item| () ,
                do_close: || (),
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

#[derive(Default)]
struct FieldsSelect {
    default: u32,
    display: select_type::Display,
}

#[derive(Default)]
struct FieldsMultiSelect {
    default: Vec<u32>,
    display: multi_select_type::Display,
}

struct ItemFields {
    name: String,
    typ: usize,
    text_type: TextType,
    checkbox_type: CheckboxType,
    select_type: FieldsSelect,
    multi_select_type: FieldsMultiSelect,
    options: Vec<SelectOption>,
}

impl Default for ItemFields {
    fn default() -> Self {
        ItemFields {
            name: String::default(),
            typ: 0,
            text_type: TextType::default(),
            checkbox_type: CheckboxType::default(),
            select_type: FieldsSelect::default(),
            multi_select_type: FieldsMultiSelect::default(),
            options: Vec::default(),
        }
    }
}

#[component]
fn NewSchemaItemModal<DoSubmit: Fn(RegistrationSchemaItem) -> (), DoClose: Fn() -> ()>(
    cx: Scope,
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let toast_manager = use_toasts(cx).unwrap();
    let fields = use_ref(cx, || ItemFields::default());
    let selects = use_const(cx, || {
        ItemFieldsType::iter()
            .map(|e| {
                let estr = format!("{}", e);
                (e, estr)
            })
            .collect::<Vec<_>>()
    });

    cx.render(rsx!(Modal {
        title: "New Field",
        do_submit: || { () },
        do_close: || { () },
        disable_submit: false,
        form {
            TextInput{
                label: "Name",
                value: fields.read().name.clone(),
                oninput: |evt: FormEvent| fields.with_mut(|fields| fields.name = evt.value.clone()),
            }
            SelectInput {
                label: "Type",
                options: selects.iter().map(|(_, estr)| estr).cloned().collect(),
                onchange: move |evt: FormEvent| {
                    let idx = match evt.value.parse::<usize>() {
                        Ok(idx) => idx,
                        Err(e) => {
                            toast_manager.borrow_mut().new_error(format!("{}", e));
                            return;
                        },
                    };
                    fields.with_mut(|fields| fields.typ = idx)
                },
                value: fields.read().typ,
            }
        }
    }))
}

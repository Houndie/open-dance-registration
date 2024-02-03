use crate::{
    components::{
        form::{Button, ButtonFlavor, CheckInput, SelectInput, TextInput, TextInputType},
        modal::Modal,
        page::Page as GenericPage,
    },
    hooks::toasts::use_toasts,
};
use common::proto::{multi_select_type, CheckboxType, RegistrationSchemaItem, SelectOption};
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
            Button {
                flavor: ButtonFlavor::Info,
                onclick: |_| show_schema_item_modal.set(true),
                "Add Field"
            }
        }
        if **show_schema_item_modal {
            rsx!(NewSchemaItemModal{
                do_submit: |item| () ,
                do_close: || show_schema_item_modal.set(false),
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

#[derive(Default)]
struct FieldsSelect {
    default: u32,
    display: usize,
}

#[derive(Default)]
struct FieldsMultiSelect {
    default: Vec<u32>,
    display: multi_select_type::Display,
}

#[derive(Default)]
struct FieldsText {
    default: String,
    display: usize,
}

struct ItemFields {
    name: String,
    typ: usize,
    text_type: FieldsText,
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
            text_type: FieldsText::default(),
            checkbox_type: CheckboxType::default(),
            select_type: FieldsSelect::default(),
            multi_select_type: FieldsMultiSelect::default(),
            options: Vec::default(),
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
    do_submit: DoSubmit,
    do_close: DoClose,
) -> Element {
    let toaster = use_toasts(cx).unwrap();
    let fields = use_ref(cx, || ItemFields::default());
    let type_selects = use_const(cx, || enum_selects::<ItemFieldsType>());
    let text_display_selects = use_const(cx, || enum_selects::<TextDisplayType>());
    let select_display_selects = use_const(cx, || enum_selects::<SelectDisplayType>());

    cx.render(rsx!(Modal {
        title: "New Field",
        do_submit: || { () },
        do_close: do_close,
        disable_submit: false,
        form {
            TextInput{
                label: "Name",
                value: TextInputType::Text(fields.read().name.clone()),
                oninput: |evt: FormEvent| fields.write().name = evt.value.clone(),
            }
            SelectInput {
                label: "Type",
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
                        fields.write().typ = idx;
                    }
                },
                value: fields.read().typ,
            }
            div {
                class: "border rounded p-3",

                match type_selects[fields.read().typ].0 {
                    ItemFieldsType::Text => rsx!(
                        TextInput{
                            label: "Default",
                            value: TextInputType::Text(fields.read().text_type.default.clone()),
                            oninput: |evt: FormEvent| fields.write().text_type.default = evt.value.clone(),
                        }
                        SelectInput {
                            label: "Display",
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
                    ),

                    ItemFieldsType::Checkbox => {
                        log::info!("B1 {}", fields.read().checkbox_type.default);
                        rsx!(
                        CheckInput{
                            label: "Default",
                            value: fields.read().checkbox_type.default,
                            onclick: |_| fields.with_mut(|fields| fields.checkbox_type.default = !fields.checkbox_type.default),
                        }
                    )},

                    ItemFieldsType::Select => rsx!(
                        SelectInput {
                            label: "Display",
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
                        fields.read().options.iter().enumerate().map(|(idx, option)| {
                            log::info!("{} {}", fields.read().select_type.default as usize, idx);
                            rsx!(
                                div {
                                    class: "d-flex",
                                    div {
                                        class: "flex-grow-1",
                                        TextInput {
                                            key: "{idx}",
                                            label: "Name",
                                            value: TextInputType::Text(option.name.clone()),
                                            oninput: move |evt: FormEvent| fields.write().options[idx].name = evt.value.clone(),
                                        }
                                    }
                                    div {
                                        class: "align-self-end ps-1",
                                        CheckInput{
                                            label: "Default",
                                            value: fields.read().select_type.default as usize == idx,
                                            onclick: move |_| {
                                                let num = match idx.try_into() {
                                                    Ok(idx) => idx,
                                                    Err(_) => {
                                                        fields.needs_update();
                                                        return;
                                                    },
                                                };

                                                fields.write().select_type.default = num;
                                            }
                                        }
                                    }
                                }
                            )
                        })
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: |_| fields.write().options.push(SelectOption::default()),
                            "Add Option"
                        }
                    ),
                    ItemFieldsType::MultiSelect => rsx!(div{}),
                }
            }
        }
    }))
}

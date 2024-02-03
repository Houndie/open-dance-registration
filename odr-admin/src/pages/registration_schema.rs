use std::collections::BTreeSet;

use crate::{
    components::{
        form::{
            Button, ButtonFlavor, CheckInput, CheckStyle, Field, SelectInput, TextInput,
            TextInputType,
        },
        modal::Modal,
        page::Page as GenericPage,
    },
    hooks::toasts::use_toasts,
};
use common::proto::{
    multi_select_type, registration_schema_item_type::Type as ItemType, select_type, text_type,
    CheckboxType, MultiSelectType, RegistrationSchemaItem, RegistrationSchemaItemType,
    SelectOption, SelectType, TextType,
};
use dioxus::prelude::*;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[component]
pub fn Page(cx: Scope, id: String) -> Element {
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

#[derive(EnumIter, strum_macros::Display)]
enum MultiSelectDisplayType {
    Checkboxes,
    MultiselectBox,
}

#[derive(Default)]
struct FieldsSelect {
    display: usize,
}

#[derive(Default)]
struct FieldsMultiSelect {
    display: usize,
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
    defaults: BTreeSet<usize>,
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
            defaults: BTreeSet::default(),
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
    let multi_select_display_selects = use_const(cx, || enum_selects::<MultiSelectDisplayType>());

    cx.render(rsx!(Modal {
        title: "New Field",
        do_submit: || {
            let field_pin = fields.read();
            let item = RegistrationSchemaItem {
                id: "".to_owned(),
                name: field_pin.name.clone(),
                r#type: Some(RegistrationSchemaItemType{
                    r#type: Some(match type_selects[field_pin.typ].0 {
                        ItemFieldsType::Text => ItemType::Text(TextType {
                            default: field_pin.text_type.default.clone(),
                            display: match text_display_selects[field_pin.text_type.display].0 {
                                TextDisplayType::Small => text_type::Display::Small,
                                TextDisplayType::Large => text_type::Display::Large,
                            } as i32,
                        }),
                        ItemFieldsType::Checkbox => ItemType::Checkbox(CheckboxType {
                            default: field_pin.checkbox_type.default,
                        }),
                        ItemFieldsType::Select => ItemType::Select(SelectType{
                            default: field_pin.defaults.first().copied().unwrap_or(0) as u32,
                            display: match select_display_selects[field_pin.select_type.display].0 {
                                SelectDisplayType::Radio => select_type::Display::Radio,
                                SelectDisplayType::Dropdown => select_type::Display::Dropdown,
                            } as i32,
                            options: field_pin.options.iter().map(|option| SelectOption{
                                id: "".to_owned(),
                                name: option.name.clone(),
                                product_id: "".to_owned(),
                            }).collect(),
                        }),
                        ItemFieldsType::MultiSelect => ItemType::MultiSelect(MultiSelectType{
                            defaults: field_pin.defaults.iter().map(|idx| *idx as u32).collect(),
                            display: match multi_select_display_selects[field_pin.multi_select_type.display].0 {
                                MultiSelectDisplayType::Checkboxes => multi_select_type::Display::Checkboxes,
                                MultiSelectDisplayType::MultiselectBox => multi_select_type::Display::MultiselectBox,
                            } as i32,
                            options: field_pin.options.iter().map(|option| SelectOption{
                                id: "".to_owned(),
                                name: option.name.clone(),
                                product_id: "".to_owned(),
                            }).collect(),
                        }),
                    }),
                }),
            };

            do_submit(item);
        },
        do_close: do_close,
        disable_submit: false,
        form {
            Field {
                label: "Name",
                TextInput{
                    value: TextInputType::Text(fields.read().name.clone()),
                    oninput: |evt: FormEvent| fields.write().name = evt.value.clone(),
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
                                fields.read().defaults.first().copied()
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
                        Field {
                            label: "",
                            CheckInput{
                                style: CheckStyle::Radio,
                                label: "No defaults",
                                value: fields.read().defaults.is_empty(),
                                onclick: |_| fields.write().defaults.clear(),
                            }
                        }
                        fields.read().options.iter().enumerate().map(|(idx, option)| {
                            rsx!{
                                Field {
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].name = evt.value.clone(),
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
                                }
                            }
                        })
                        Button {
                            flavor: ButtonFlavor::Info,
                            onclick: |_| fields.write().options.push(SelectOption::default()),
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
                                    label: "Name",
                                    TextInput{
                                        value: TextInputType::Text(option.name.clone()),
                                        is_expanded: true,
                                        oninput: move |evt: FormEvent| fields.write().options[idx].name = evt.value.clone(),
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
                            onclick: |_| fields.write().options.push(SelectOption::default()),
                            "Add Option"
                        }
                    },
                }
            }
        }
    }))
}

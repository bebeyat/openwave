//! "Kendi CSS'in" card in Özelleştirme: a stylesheet the user writes or loads from a file, applied
//! after the app's own rules (see `boot::user_css_script`).

use dioxus::prelude::*;

use super::rows::Card;
use crate::i18n::t;
use crate::store::settings::USER_CSS_MAX;
use crate::store::SettingsChange;
use crate::ui::{toast_error, Button, ButtonVariant};

async fn load_file() -> Option<String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("CSS", &["css"])
        .pick_file()
        .await?;
    match tokio::fs::read_to_string(file.path()).await {
        Ok(text) if text.len() <= USER_CSS_MAX => Some(text),
        Ok(_) => {
            toast_error(t("custom.css.too_big"));
            None
        }
        Err(e) => {
            toast_error(e.to_string());
            None
        }
    }
}

#[component]
pub fn UserCssCard(saved: String, on_change: EventHandler<SettingsChange>) -> Element {
    let mut draft = use_signal(|| saved.clone());
    let changed = draft() != saved;
    rsx! {
        Card { class: "py-3",
            textarea {
                class: "h-48 w-full resize-y rounded-md border border-line-strong bg-surface-2 p-3 font-mono text-xs text-fg outline-none focus-visible:outline-2 focus-visible:outline-accent-text",
                "aria-label": t("custom.css"),
                spellcheck: "false",
                placeholder: ".ow-panel {{ border-radius: 0; }}",
                maxlength: "{USER_CSS_MAX}",
                value: "{draft}",
                oninput: move |e| draft.set(e.value()),
            }
            div { class: "mt-2 flex flex-wrap justify-end gap-2",
                Button {
                    variant: ButtonVariant::Ghost,
                    icon: "folder-open",
                    onclick: move |_| {
                        spawn(async move {
                            if let Some(text) = load_file().await {
                                draft.set(text);
                            }
                        });
                    },
                    {t("custom.css.load")}
                }
                Button {
                    variant: ButtonVariant::Ghost,
                    icon: "trash-2",
                    disabled: draft().is_empty() && saved.is_empty(),
                    onclick: move |_| {
                        draft.set(String::new());
                        on_change.call(SettingsChange::UserCss(String::new()));
                    },
                    {t("custom.css.clear")}
                }
                Button {
                    variant: ButtonVariant::Primary,
                    icon: "check",
                    disabled: !changed,
                    onclick: move |_| on_change.call(SettingsChange::UserCss(draft())),
                    {t("custom.css.apply")}
                }
            }
        }
    }
}

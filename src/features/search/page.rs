//! `/search?q=`: debounced catalog search with tabs, recent searches and suggestions.

use std::sync::Arc;
use std::time::Duration;

use dioxus::dioxus_core::Task;
use dioxus::prelude::*;

use super::idle::IdleView;
use super::logic::{normalize_query, Phase, RequestGuard, Tab, DEBOUNCE_MS, REMEMBER_AFTER_MS};
use super::results::{SearchResultsView, SearchStatus};
use crate::backend::{api, Coded};
use crate::features::shell::pages::Page;
use crate::features::tracks::state::remember_search;
use crate::i18n::t;
use crate::ui::{TabItem, TabPanel, Tabs};

const GROUP: &str = "search";

/// Runs the debounced, cancellable search; returns the phase the page should show.
fn use_search(q: String, tab: Tab, nonce: u64) -> Signal<Phase> {
    let term = normalize_query(&q);
    let mut phase = use_signal(|| {
        if term.is_empty() {
            Phase::Idle
        } else {
            Phase::Loading
        }
    });
    let guard = use_hook(RequestGuard::default);
    let mut task = use_signal(|| None::<Task>);
    use_effect(use_reactive!(|term, tab, nonce| {
        let _ = nonce;
        let ticket = guard.advance();
        if let Some(old) = task.write().take() {
            old.cancel();
        }
        if term.is_empty() {
            phase.set(Phase::Idle);
            return;
        }
        phase.set(Phase::Loading);
        let guard = guard.clone();
        let handle = spawn(async move {
            tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS)).await;
            if !guard.is_current(ticket) {
                return;
            }
            let result = api::search(tab.kind(), &term).await;
            if !guard.is_current(ticket) {
                return;
            }
            match result {
                Ok(r) => {
                    let found = tab.count(&r) > 0;
                    phase.set(Phase::Done(Arc::new(r)));
                    if found {
                        tokio::time::sleep(Duration::from_millis(REMEMBER_AFTER_MS)).await;
                        if guard.is_current(ticket) {
                            remember_search(&term);
                        }
                    }
                }
                Err(e) => phase.set(Phase::Failed(e.user_message())),
            }
        });
        task.set(Some(handle));
    }));
    phase
}

#[component]
pub fn Search(q: String) -> Element {
    let mut tab = use_signal(Tab::default);
    let mut nonce = use_signal(|| 0u64);
    let phase = use_search(q.clone(), tab(), nonce());
    let idle = normalize_query(&q).is_empty();
    let items: Vec<TabItem> = Tab::ALL
        .iter()
        .map(|t_| TabItem::new(t_.value(), t(t_.label_key())))
        .collect();
    rsx! {
        Page { title: t("search.title"),
            SearchStatus { tab: tab(), phase: phase() }
            if idle {
                IdleView {}
            } else {
                div { class: "ow-enter",
                Tabs {
                    items,
                    value: tab().value(),
                    group: GROUP,
                    aria_label: t("search.title"),
                    class: "ow-seg mb-6",
                    on_change: move |v: String| tab.set(Tab::parse(&v)),
                }
                TabPanel { value: tab().value(), group: GROUP, focusable: false,
                    SearchResultsView {
                        tab: tab(),
                        phase: phase(),
                        on_retry: move |_| nonce += 1,
                    }
                }
                }
            }
        }
    }
}

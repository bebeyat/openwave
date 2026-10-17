//! Discord "Listening to" status (Ayarlar › Genel, opt-in).
//!
//! A worker thread owns the IPC connection to the local Discord client. The UI side
//! (`use_discord_presence`, called once in `App`) watches the player and sends the wanted activity
//! only when it really changes (`presence::worth_sending`). When Discord is not running the
//! worker keeps the last wish and retries every [`RETRY`], so starting Discord later just works.

pub mod presence;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::OnceLock;
use std::time::Duration;

use dioxus::prelude::*;
use discord_rich_presence::{
    activity::{Activity, ActivityType, Assets, Button, StatusDisplayType, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

use crate::features::settings::boot::DISCORD;
use crate::i18n::t;
use crate::player::controller as ctl;
use crate::store::library::now_ms;
use presence::{view, worth_sending, NowPlaying, PresenceView};

const RETRY: Duration = Duration::from_secs(15);
/// OpenWave's Discord application (its name is what Discord shows as "Listening to …").
const APP_ID: &str = "1561132308793270312";

/// What the UI wants Discord to show.
#[derive(Clone, Debug, PartialEq)]
struct Wish {
    app_id: String,
    view: Option<PresenceView>,
    button_label: String,
}

fn worker() -> &'static Sender<Wish> {
    static TX: OnceLock<Sender<Wish>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        // A missing thread only means no Discord status; the app keeps working.
        let _ = std::thread::Builder::new()
            .name("discord-presence".into())
            .spawn(move || run(rx));
        tx
    })
}

fn activity<'a>(v: &'a PresenceView, button_label: &'a str) -> Activity<'a> {
    let mut a = Activity::new()
        .activity_type(ActivityType::Listening)
        .status_display_type(StatusDisplayType::Details)
        .details(v.details.as_str())
        .state(v.state.as_str());
    let mut assets = Assets::new();
    if let Some(img) = &v.large_image {
        assets = assets.large_image(img.as_str());
    }
    if let Some(text) = &v.large_text {
        assets = assets.large_text(text.as_str());
    }
    a = a.assets(assets);
    if let Some(start) = v.start_ms {
        let mut ts = Timestamps::new().start(start);
        if let Some(end) = v.end_ms {
            ts = ts.end(end);
        }
        a = a.timestamps(ts);
    }
    if let Some(url) = &v.button_url {
        a = a.buttons(vec![Button::new(button_label, url.as_str())]);
    }
    a
}

/// Apply `wish` on `client` (connecting first when needed). `false` when Discord did not take it.
fn apply(client: &mut Option<(String, DiscordIpcClient)>, wish: &Wish) -> bool {
    if wish.app_id.is_empty() {
        if let Some((_, mut c)) = client.take() {
            let _ = c.clear_activity();
            let _ = c.close();
        }
        return true;
    }
    if client.as_ref().is_some_and(|(id, _)| *id != wish.app_id) {
        if let Some((_, mut c)) = client.take() {
            let _ = c.close();
        }
    }
    if client.is_none() {
        let mut c = DiscordIpcClient::new(&wish.app_id);
        if c.connect().is_err() {
            return false;
        }
        *client = Some((wish.app_id.clone(), c));
    }
    let Some((_, c)) = client.as_mut() else {
        return false;
    };
    let sent = match &wish.view {
        Some(v) => c.set_activity(activity(v, &wish.button_label)),
        None => c.clear_activity(),
    };
    if sent.is_err() {
        // Discord closed or restarted: reconnect on the next attempt.
        if let Some((_, mut c)) = client.take() {
            let _ = c.close();
        }
        return false;
    }
    true
}

fn run(rx: Receiver<Wish>) {
    let mut client: Option<(String, DiscordIpcClient)> = None;
    let mut pending: Option<Wish> = None;
    loop {
        match rx.recv_timeout(RETRY) {
            Ok(wish) => pending = Some(wish),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // Only the newest wish matters.
        while let Ok(wish) = rx.try_recv() {
            pending = Some(wish);
        }
        if let Some(wish) = pending.as_ref() {
            if apply(&mut client, wish) {
                pending = None;
            }
        }
    }
    if let Some((_, mut c)) = client {
        let _ = c.clear_activity();
        let _ = c.close();
    }
}

fn now_playing() -> Option<NowPlaying> {
    let queue = ctl::QUEUE.read();
    let track = queue.current()?.track.clone();
    drop(queue);
    Some(NowPlaying {
        id: track.id.to_string(),
        title: track.title.to_string(),
        artist: track.artists_label(),
        album: track.album.as_ref().map(|a| a.title.to_string()),
        cover: track.cover.as_ref().map(|c| c.url.to_string()),
        duration_s: ctl::display_duration(),
        position_s: *ctl::POSITION.read(),
        playing: ctl::STATUS.read().is_active(),
    })
}

/// Call once near the root.
pub fn use_discord_presence() {
    let last = use_hook(|| Rc::new(RefCell::new((None::<String>, None::<PresenceView>))));
    use_effect(move || {
        let wanted_id = if *DISCORD.read() {
            APP_ID.to_owned()
        } else {
            String::new()
        };
        let next = now_playing().and_then(|np| view(&np, now_ms() as i64));
        let next = if wanted_id.is_empty() { None } else { next };
        let mut last = last.borrow_mut();
        // Never enabled: no message, so the worker thread is not even started.
        if wanted_id.is_empty() && last.0.is_none() {
            return;
        }
        let id_changed = last.0.as_deref() != Some(wanted_id.as_str());
        if !id_changed && !worth_sending(last.1.as_ref(), next.as_ref()) {
            return;
        }
        *last = (Some(wanted_id.clone()), next.clone());
        let _ = worker().send(Wish {
            app_id: wanted_id,
            view: next,
            button_label: t("discord.listen_button"),
        });
    });
}

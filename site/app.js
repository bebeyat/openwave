// OpenWave share page: reads a shared list from public Nostr relays in the browser and offers to
// open it in the app. Mirrors src/backend/share.rs: the id after "#" is 10 base62 characters of the
// SHA-256 of the event content, so only content that hashes to the id is shown.
"use strict";

const RELAYS = ["wss://relay.damus.io", "wss://nos.lol", "wss://relay.primal.net", "wss://nostr.mom", "wss://relay.nostr.net"];
const KIND = 30078;
const TIMEOUT_MS = 8000;
const B62 = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const IMG_HOSTS = ["ytimg.com", "ggpht.com", "googleusercontent.com"];
const RELEASES = "https://github.com/bebeyat/openwave/releases/latest";
const APP_KEY = "ow-has-app";

const STRINGS = {
  en: { loading: "Loading…", list: "Shared playlist", song: "Shared song", open: "Open in OpenWave", get: "Get OpenWave", note: "A free, ad-free music player for Windows, macOS and Linux.", hint: "Nothing happened? Get OpenWave, then open the link again.", foot: "Songs play from YouTube Music. No account, no ads.", privacy: "Privacy", missing_t: "This link can't be found", missing_b: "The list may have been removed, or the link is incomplete. Ask for a new one.", offline_t: "Couldn't load this list", offline_b: "Check your connection and refresh the page.", nohash_t: "Share playlists with one link", nohash_b: "Make a playlist in OpenWave, press Share and send the link to a friend.", ytm: "Listen on YouTube Music", one: "{n} song", other: "{n} songs" },
  tr: { loading: "Yükleniyor…", list: "Paylaşılan liste", song: "Paylaşılan şarkı", open: "OpenWave'de aç", get: "OpenWave'i indir", note: "Windows, macOS ve Linux için ücretsiz ve reklamsız müzik çalar.", hint: "Bir şey olmadı mı? OpenWave'i indir, sonra bağlantıyı yeniden aç.", foot: "Şarkılar YouTube Music'ten çalar. Hesap yok, reklam yok.", privacy: "Gizlilik", missing_t: "Bu bağlantı bulunamadı", missing_b: "Liste kaldırılmış ya da bağlantı eksik olabilir. Yeni bir bağlantı iste.", offline_t: "Liste yüklenemedi", offline_b: "Bağlantını kontrol edip sayfayı yenile.", nohash_t: "Listeleri tek bağlantıyla paylaş", nohash_b: "OpenWave'de bir liste yap, Paylaş'a bas ve bağlantıyı arkadaşına gönder.", ytm: "YouTube Music'te dinle", one: "{n} şarkı", other: "{n} şarkı" },
  de: { loading: "Wird geladen…", list: "Geteilte Playlist", song: "Geteilter Song", open: "In OpenWave öffnen", get: "OpenWave holen", note: "Ein kostenloser, werbefreier Musikplayer für Windows, macOS und Linux.", hint: "Nichts passiert? Hol dir OpenWave und öffne den Link dann erneut.", foot: "Songs werden von YouTube Music abgespielt. Kein Konto, keine Werbung.", privacy: "Datenschutz", missing_t: "Dieser Link wurde nicht gefunden", missing_b: "Die Liste wurde vielleicht entfernt oder der Link ist unvollständig. Bitte um einen neuen.", offline_t: "Liste konnte nicht geladen werden", offline_b: "Prüfe deine Verbindung und lade die Seite neu.", nohash_t: "Playlists mit einem Link teilen", nohash_b: "Erstelle eine Playlist in OpenWave, tippe auf Teilen und schick den Link an Freunde.", ytm: "Auf YouTube Music anhören", one: "{n} Song", other: "{n} Songs" },
  es: { loading: "Cargando…", list: "Lista compartida", song: "Canción compartida", open: "Abrir en OpenWave", get: "Descargar OpenWave", note: "Un reproductor de música gratis y sin anuncios para Windows, macOS y Linux.", hint: "¿No pasó nada? Descarga OpenWave y vuelve a abrir el enlace.", foot: "Las canciones suenan desde YouTube Music. Sin cuenta, sin anuncios.", privacy: "Privacidad", missing_t: "No se encontró este enlace", missing_b: "Puede que la lista se haya eliminado o que el enlace esté incompleto. Pide uno nuevo.", offline_t: "No se pudo cargar la lista", offline_b: "Revisa tu conexión y recarga la página.", nohash_t: "Comparte listas con un enlace", nohash_b: "Crea una lista en OpenWave, pulsa Compartir y envía el enlace a un amigo.", ytm: "Escuchar en YouTube Music", one: "{n} canción", other: "{n} canciones" },
  fr: { loading: "Chargement…", list: "Playlist partagée", song: "Titre partagé", open: "Ouvrir dans OpenWave", get: "Obtenir OpenWave", note: "Un lecteur de musique gratuit et sans pub pour Windows, macOS et Linux.", hint: "Rien ne se passe ? Installe OpenWave, puis rouvre le lien.", foot: "Les titres sont lus depuis YouTube Music. Sans compte, sans pub.", privacy: "Confidentialité", missing_t: "Ce lien est introuvable", missing_b: "La liste a peut-être été supprimée ou le lien est incomplet. Demande-en un nouveau.", offline_t: "Impossible de charger la liste", offline_b: "Vérifie ta connexion et recharge la page.", nohash_t: "Partage des playlists en un lien", nohash_b: "Crée une playlist dans OpenWave, appuie sur Partager et envoie le lien à un ami.", ytm: "Écouter sur YouTube Music", one: "{n} titre", other: "{n} titres" },
  id: { loading: "Memuat…", list: "Playlist yang dibagikan", song: "Lagu yang dibagikan", open: "Buka di OpenWave", get: "Dapatkan OpenWave", note: "Pemutar musik gratis tanpa iklan untuk Windows, macOS, dan Linux.", hint: "Tidak terjadi apa-apa? Pasang OpenWave, lalu buka tautannya lagi.", foot: "Lagu diputar dari YouTube Music. Tanpa akun, tanpa iklan.", privacy: "Privasi", missing_t: "Tautan ini tidak ditemukan", missing_b: "Daftarnya mungkin sudah dihapus atau tautannya tidak lengkap. Minta tautan baru.", offline_t: "Daftar tidak dapat dimuat", offline_b: "Periksa koneksimu lalu muat ulang halaman.", nohash_t: "Bagikan playlist dengan satu tautan", nohash_b: "Buat playlist di OpenWave, tekan Bagikan, lalu kirim tautannya ke teman.", ytm: "Dengarkan di YouTube Music", one: "{n} lagu", other: "{n} lagu" },
  it: { loading: "Caricamento…", list: "Playlist condivisa", song: "Brano condiviso", open: "Apri in OpenWave", get: "Scarica OpenWave", note: "Un lettore musicale gratuito e senza pubblicità per Windows, macOS e Linux.", hint: "Non succede niente? Installa OpenWave e riapri il link.", foot: "I brani vengono riprodotti da YouTube Music. Nessun account, nessuna pubblicità.", privacy: "Privacy", missing_t: "Link non trovato", missing_b: "La lista potrebbe essere stata rimossa o il link è incompleto. Chiedine uno nuovo.", offline_t: "Impossibile caricare la lista", offline_b: "Controlla la connessione e ricarica la pagina.", nohash_t: "Condividi playlist con un link", nohash_b: "Crea una playlist in OpenWave, premi Condividi e invia il link a un amico.", ytm: "Ascolta su YouTube Music", one: "{n} brano", other: "{n} brani" },
  ja: { loading: "読み込み中…", list: "共有されたプレイリスト", song: "共有された曲", open: "OpenWave で開く", get: "OpenWave を入手", note: "Windows・macOS・Linux 向けの無料・広告なしの音楽プレイヤー。", hint: "何も起きませんか？ OpenWave を入れてから、もう一度リンクを開いてください。", foot: "曲は YouTube Music から再生されます。アカウント不要、広告なし。", privacy: "プライバシー", missing_t: "このリンクは見つかりません", missing_b: "リストが削除されたか、リンクが不完全な可能性があります。新しいリンクを送ってもらってください。", offline_t: "リストを読み込めませんでした", offline_b: "接続を確認してページを再読み込みしてください。", nohash_t: "リンクひとつでプレイリストを共有", nohash_b: "OpenWave でプレイリストを作り、「共有」を押して友だちにリンクを送りましょう。", ytm: "YouTube Music で聴く", one: "{n} 曲", other: "{n} 曲" },
  ko: { loading: "불러오는 중…", list: "공유된 플레이리스트", song: "공유된 곡", open: "OpenWave에서 열기", get: "OpenWave 받기", note: "Windows, macOS, Linux용 무료 광고 없는 음악 플레이어.", hint: "아무 일도 없나요? OpenWave를 설치한 뒤 링크를 다시 여세요.", foot: "곡은 YouTube Music에서 재생됩니다. 계정도 광고도 없습니다.", privacy: "개인정보", missing_t: "링크를 찾을 수 없습니다", missing_b: "목록이 삭제되었거나 링크가 불완전할 수 있습니다. 새 링크를 요청하세요.", offline_t: "목록을 불러올 수 없습니다", offline_b: "연결을 확인하고 페이지를 새로 고치세요.", nohash_t: "링크 하나로 플레이리스트 공유", nohash_b: "OpenWave에서 플레이리스트를 만들고 공유를 눌러 친구에게 링크를 보내세요.", ytm: "YouTube Music에서 듣기", one: "{n}곡", other: "{n}곡" },
  nl: { loading: "Laden…", list: "Gedeelde playlist", song: "Gedeeld nummer", open: "Openen in OpenWave", get: "OpenWave downloaden", note: "Een gratis muziekspeler zonder advertenties voor Windows, macOS en Linux.", hint: "Gebeurt er niets? Installeer OpenWave en open de link opnieuw.", foot: "Nummers worden afgespeeld via YouTube Music. Geen account, geen advertenties.", privacy: "Privacy", missing_t: "Deze link is niet gevonden", missing_b: "De lijst is misschien verwijderd of de link is onvolledig. Vraag om een nieuwe.", offline_t: "Lijst kon niet worden geladen", offline_b: "Controleer je verbinding en vernieuw de pagina.", nohash_t: "Deel playlists met één link", nohash_b: "Maak een playlist in OpenWave, tik op Delen en stuur de link naar een vriend.", ytm: "Luisteren op YouTube Music", one: "{n} nummer", other: "{n} nummers" },
  pl: { loading: "Ładowanie…", list: "Udostępniona playlista", song: "Udostępniony utwór", open: "Otwórz w OpenWave", get: "Pobierz OpenWave", note: "Darmowy odtwarzacz muzyki bez reklam dla Windows, macOS i Linux.", hint: "Nic się nie stało? Zainstaluj OpenWave i otwórz link ponownie.", foot: "Utwory są odtwarzane z YouTube Music. Bez konta, bez reklam.", privacy: "Prywatność", missing_t: "Nie znaleziono linku", missing_b: "Lista mogła zostać usunięta albo link jest niepełny. Poproś o nowy.", offline_t: "Nie udało się wczytać listy", offline_b: "Sprawdź połączenie i odśwież stronę.", nohash_t: "Udostępniaj playlisty jednym linkiem", nohash_b: "Utwórz playlistę w OpenWave, kliknij Udostępnij i wyślij link znajomemu.", ytm: "Słuchaj w YouTube Music", one: "{n} utwór", few: "{n} utwory", many: "{n} utworów", other: "{n} utworu" },
  pt: { loading: "Carregando…", list: "Playlist compartilhada", song: "Música compartilhada", open: "Abrir no OpenWave", get: "Baixar o OpenWave", note: "Um player de música gratuito e sem anúncios para Windows, macOS e Linux.", hint: "Nada aconteceu? Instale o OpenWave e abra o link de novo.", foot: "As músicas tocam pelo YouTube Music. Sem conta, sem anúncios.", privacy: "Privacidade", missing_t: "Link não encontrado", missing_b: "A lista pode ter sido removida ou o link está incompleto. Peça um novo.", offline_t: "Não foi possível carregar a lista", offline_b: "Verifique sua conexão e recarregue a página.", nohash_t: "Compartilhe playlists com um link", nohash_b: "Crie uma playlist no OpenWave, toque em Compartilhar e envie o link para um amigo.", ytm: "Ouvir no YouTube Music", one: "{n} música", other: "{n} músicas" },
  ru: { loading: "Загрузка…", list: "Плейлист", song: "Трек", open: "Открыть в OpenWave", get: "Скачать OpenWave", note: "Бесплатный музыкальный плеер без рекламы для Windows, macOS и Linux.", hint: "Ничего не произошло? Установите OpenWave и откройте ссылку снова.", foot: "Треки играют из YouTube Music. Без аккаунта и рекламы.", privacy: "Конфиденциальность", missing_t: "Ссылка не найдена", missing_b: "Список мог быть удалён, или ссылка неполная. Попросите новую.", offline_t: "Не удалось загрузить список", offline_b: "Проверьте подключение и обновите страницу.", nohash_t: "Делитесь плейлистами одной ссылкой", nohash_b: "Создайте плейлист в OpenWave, нажмите «Поделиться» и отправьте ссылку другу.", ytm: "Слушать в YouTube Music", one: "{n} трек", few: "{n} трека", many: "{n} треков", other: "{n} трека" },
  uk: { loading: "Завантаження…", list: "Плейлист", song: "Трек", open: "Відкрити в OpenWave", get: "Завантажити OpenWave", note: "Безкоштовний музичний плеєр без реклами для Windows, macOS і Linux.", hint: "Нічого не сталося? Встановіть OpenWave і відкрийте посилання знову.", foot: "Треки грають із YouTube Music. Без облікового запису й реклами.", privacy: "Конфіденційність", missing_t: "Посилання не знайдено", missing_b: "Список могли видалити, або посилання неповне. Попросіть нове.", offline_t: "Не вдалося завантажити список", offline_b: "Перевірте з'єднання й оновіть сторінку.", nohash_t: "Діліться плейлистами одним посиланням", nohash_b: "Створіть плейлист в OpenWave, натисніть «Поділитися» й надішліть посилання другові.", ytm: "Слухати в YouTube Music", one: "{n} трек", few: "{n} треки", many: "{n} треків", other: "{n} треку" },
  zh: { loading: "加载中…", list: "分享的歌单", song: "分享的歌曲", open: "在 OpenWave 中打开", get: "获取 OpenWave", note: "适用于 Windows、macOS 和 Linux 的免费无广告音乐播放器。", hint: "没有反应？先安装 OpenWave，再重新打开链接。", foot: "歌曲来自 YouTube Music。无需账号，没有广告。", privacy: "隐私", missing_t: "找不到这个链接", missing_b: "歌单可能已被删除，或链接不完整。请让对方重新发送。", offline_t: "无法加载歌单", offline_b: "请检查网络后刷新页面。", nohash_t: "一个链接分享歌单", nohash_b: "在 OpenWave 中创建歌单，点击分享，把链接发给朋友。", ytm: "在 YouTube Music 上收听", one: "{n} 首歌", other: "{n} 首歌" },
};

const lang = (() => {
  for (const l of navigator.languages || [navigator.language || "en"]) {
    const code = String(l).slice(0, 2).toLowerCase();
    if (STRINGS[code]) return code;
  }
  return "en";
})();
const S = STRINGS[lang];
const $ = (id) => document.getElementById(id);

function el(tag, props, ...kids) {
  const e = document.createElement(tag);
  Object.assign(e, props);
  for (const k of kids) e.append(k);
  return e;
}

function count(n) {
  const form = new Intl.PluralRules(lang).select(n);
  return (S[form] || S.other).replace("{n}", new Intl.NumberFormat(lang).format(n));
}

function totalTime(ms) {
  const mins = Math.round(ms / 60000);
  const unit = (u, v) => new Intl.NumberFormat(lang, { style: "unit", unit: u, unitDisplay: "short" }).format(v);
  return mins >= 60 ? `${unit("hour", Math.floor(mins / 60))} ${unit("minute", mins % 60)}` : unit("minute", mins);
}

function clock(ms) {
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function safeImage(url) {
  try {
    const u = new URL(url);
    return u.protocol === "https:" && IMG_HOSTS.some((h) => u.hostname === h || u.hostname.endsWith("." + h)) ? u.href : null;
  } catch {
    return null;
  }
}

async function idOf(content) {
  const hash = new Uint8Array(await crypto.subtle.digest("SHA-256", new TextEncoder().encode(content)));
  let n = 0n;
  for (const b of hash.slice(0, 8)) n = (n << 8n) | BigInt(b);
  let out = "";
  for (let i = 0; i < 10; i++) {
    out = B62[Number(n % 62n)] + out;
    n /= 62n;
  }
  return out;
}

async function decode(id, content) {
  if (typeof content !== "string" || content.length > 900000 || (await idOf(content)) !== id) return null;
  const bin = atob(content.replace(/-/g, "+").replace(/_/g, "/"));
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("deflate-raw"));
  const p = JSON.parse(await new Response(stream).text());
  const tracks = (Array.isArray(p.t) ? p.t : []).filter((t) => t && /^[\w-]{1,64}$/.test(String(t.id)) && !String(t.id).startsWith("local-"));
  return p.v === 1 && tracks.length ? { name: String(p.n || ""), single: !!p.s, tracks } : null;
}

// Ask every relay; the first content that matches the id wins. "missing" only if a relay answered.
function fetchShare(id) {
  return new Promise((resolve) => {
    let pending = RELAYS.length;
    let answered = false;
    let done = false;
    const sockets = [];
    const finish = (result) => {
      if (done) return;
      done = true;
      sockets.forEach((ws) => { try { ws.close(); } catch {} });
      resolve(result);
    };
    const settle = () => { if (--pending === 0) finish(answered ? { missing: true } : { offline: true }); };
    setTimeout(() => finish(answered ? { missing: true } : { offline: true }), TIMEOUT_MS);
    for (const url of RELAYS) {
      let ws;
      try { ws = new WebSocket(url); } catch { settle(); continue; }
      sockets.push(ws);
      let over = false;
      const end = () => { if (!over) { over = true; settle(); } };
      ws.onopen = () => ws.send(JSON.stringify(["REQ", "ow", { kinds: [KIND], "#d": ["ow:" + id], limit: 5 }]));
      ws.onmessage = async (m) => {
        let d;
        try { d = JSON.parse(m.data); } catch { return; }
        if (d[0] === "EVENT" && d[2]) {
          try {
            const shared = await decode(id, d[2].content);
            if (shared) finish({ shared });
          } catch {}
        } else if (d[0] === "EOSE" || d[0] === "CLOSED") {
          answered = true;
          end();
        }
      };
      ws.onerror = end;
      ws.onclose = end;
    }
  });
}

function downloadLink() {
  const ua = navigator.userAgent;
  const file = /Windows/.test(ua) ? "OpenWave-windows-x64-setup.exe" : /Mac OS X/.test(ua) && !/Mobile/.test(ua) ? "OpenWave-macos-arm64.zip" : /Linux/.test(ua) && !/Android/.test(ua) ? "OpenWave-linux-x86_64.tar.gz" : null;
  return file ? `${RELEASES}/download/${file}` : RELEASES;
}

function showEmpty(title, body) {
  $("hero").hidden = true;
  $("empty").hidden = false;
  $("empty-title").textContent = title;
  $("empty-body").textContent = body;
}

function render(id, { name, single, tracks }) {
  const kind = single ? S.song : S.list;
  const title = single ? tracks[0].title : name;
  document.title = `${title} · OpenWave`;
  $("hero").setAttribute("aria-busy", "false");
  $("kind").textContent = kind;
  $("title").textContent = title;
  const artists = (t) => (Array.isArray(t.artists) ? t.artists.map((a) => a && a.name).filter(Boolean).join(", ") : "");
  const ms = tracks.reduce((sum, t) => sum + (Number(t.duration_ms) || 0), 0);
  $("meta").textContent = single ? [artists(tracks[0]), ms ? clock(ms) : ""].filter(Boolean).join(" · ") : [count(tracks.length), ms ? totalTime(ms) : ""].filter(Boolean).join(" · ");

  const covers = [...new Set(tracks.map((t) => t.cover && safeImage(t.cover.url)).filter(Boolean))];
  const art = $("art");
  art.replaceChildren();
  const tiles = covers.length >= 4 && !single ? covers.slice(0, 4) : covers.slice(0, 1);
  art.classList.toggle("one", tiles.length === 1);
  for (const src of tiles) art.append(el("img", { src: src.replace(/=w\d+-h\d+/, "=w544-h544"), alt: "", referrerPolicy: "no-referrer" }));

  const open = $("open");
  open.href = `openwave://s/${id}`;
  open.addEventListener("click", () => {
    try { localStorage.setItem(APP_KEY, "1"); } catch {}
    setTimeout(() => { if (document.hasFocus()) $("hint").hidden = false; }, 1600);
  });
  $("actions").hidden = false;

  if (!single) {
    const list = $("tracks");
    for (const t of tracks) {
      const cover = t.cover && safeImage(t.cover.url);
      const link = el("a", { href: `https://music.youtube.com/watch?v=${encodeURIComponent(t.id)}`, rel: "noopener", title: S.ytm },
        el("img", { className: "thumb", src: cover || "icon.svg", alt: "", loading: "lazy", referrerPolicy: "no-referrer" }),
        el("span", { className: "t" }, el("b", { textContent: String(t.title || "") }), el("span", { textContent: artists(t) })),
        el("span", { className: "dur", textContent: t.duration_ms ? clock(Number(t.duration_ms)) : "" }));
      list.append(el("li", {}, link));
    }
    list.hidden = false;
  }

  // Once someone has opened a link in the app from this browser, open the next ones right away.
  let hasApp = false;
  try { hasApp = localStorage.getItem(APP_KEY) === "1"; } catch {}
  if (hasApp) location.href = open.href;
}

async function main() {
  document.documentElement.lang = lang;
  for (const node of document.querySelectorAll("[data-i18n]")) node.textContent = S[node.dataset.i18n];
  const dl = downloadLink();
  for (const a of document.querySelectorAll('a[data-i18n="get"]')) a.href = dl;

  const id = location.hash.slice(1);
  if (!/^[0-9A-Za-z]{10}$/.test(id)) return showEmpty(S.nohash_t, S.nohash_b);
  const r = await fetchShare(id);
  if (r.shared) render(id, r.shared);
  else if (r.missing) showEmpty(S.missing_t, S.missing_b);
  else showEmpty(S.offline_t, S.offline_b);
}

window.addEventListener("hashchange", () => location.reload());
main();

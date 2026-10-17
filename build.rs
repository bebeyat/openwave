//! Generates the icon lookup used by `ui::Icon`.
//!
//! Every `assets/icons/<name>.svg` becomes an entry in `icons.rs` (written to OUT_DIR):
//! no registry has to be edited to add an icon. Drop a Lucide SVG into `assets/icons`
//! (or add its name to `tools/icons.txt` and run `npm run assets`) and rebuild.
//! Only the inner markup of each SVG is embedded; `Icon` supplies the `<svg>` wrapper.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

fn inner_markup(svg: &str) -> Option<&str> {
    let open_end = svg.find('>')? + 1;
    let close = svg.rfind("</svg>")?;
    svg.get(open_end..close).map(str::trim)
}

/// Puts `assets/branding/openwave.ico` into the Windows executable (Explorer, taskbar, installer).
#[cfg(windows)]
fn embed_exe_icon() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/branding/openwave.ico");
    res.set("ProductName", "OpenWave");
    res.set("FileDescription", "OpenWave music player");
    if let Err(e) = res.compile() {
        // A missing resource compiler must not block building from source.
        println!("cargo:warning=could not embed the exe icon: {e}");
    }
}

#[cfg(not(windows))]
fn embed_exe_icon() {}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/icons");
    println!("cargo:rerun-if-changed=assets/branding/openwave.ico");
    embed_exe_icon();

    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let icons_dir = manifest.join("assets").join("icons");
    let inner_dir = out.join("icon-bodies");
    fs::create_dir_all(&inner_dir).expect("create icon-bodies dir");

    let mut names: Vec<String> = fs::read_dir(&icons_dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter_map(|e| {
                    let p = e.path();
                    (p.extension().and_then(|x| x.to_str()) == Some("svg"))
                        .then(|| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
                        .flatten()
                })
                .collect()
        })
        .unwrap_or_default();
    names.sort();

    let mut code = String::from(
        "#[cfg(test)]
pub const ICON_NAMES: &[&str] = &[\n",
    );
    for n in &names {
        let _ = writeln!(code, "    {n:?},");
    }
    code.push_str("];\n\n/// Inner SVG markup for a Lucide icon name, if bundled.\npub fn icon_body(name: &str) -> Option<&'static str> {\n    match name {\n");
    for n in &names {
        let svg = fs::read_to_string(icons_dir.join(format!("{n}.svg"))).expect("read icon");
        let body = inner_markup(&svg).unwrap_or_else(|| panic!("icon {n} is not a valid svg"));
        let dest = inner_dir.join(format!("{n}.svg.inner"));
        fs::write(&dest, body).expect("write icon body");
        let _ = writeln!(
            code,
            "        {n:?} => Some(include_str!({:?})),",
            dest.to_string_lossy().replace('\\', "/")
        );
    }
    code.push_str("        _ => None,\n    }\n}\n");
    fs::write(out.join("icons.rs"), code).expect("write icons.rs");
}

//! Checks device plugins and renders each view with its hotspots and inputs to PNG:
//! `cargo run -p gob-devices --example device_check <plugin dir>... [--out <dir>] [--light]`
//! (PNGs go to `target/device_check/<id>-<view>.png` by default). Exits with 1 on errors.
use std::path::PathBuf;

use gob_devices::{DevicePlugin, Level, Palette, Side, check, themed};
use resvg::{tiny_skia, usvg};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = PathBuf::from("target/device_check");
    let mut dirs = Vec::new();
    let mut light = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--out" => out = it.next().expect("--out <dir>").into(),
            "--light" => light = true,
            dir => dirs.push(PathBuf::from(dir)),
        }
    }
    if dirs.is_empty() {
        eprintln!("usage: device_check <plugin dir>... [--out <dir>] [--light]");
        std::process::exit(2);
    }
    std::fs::create_dir_all(&out).expect("output folder");
    let palette = if light {
        Palette::light()
    } else {
        Palette::dark()
    };
    let mut failed = false;
    for dir in dirs {
        let plugin = match DevicePlugin::load(&dir) {
            Ok(p) => p,
            Err(e) => {
                println!("{}: {e}", dir.display());
                failed = true;
                continue;
            }
        };
        let issues = check(&plugin);
        println!(
            "{} ({} controls): {} errors, {} warnings",
            plugin.id(),
            plugin.manifest.controls.len(),
            issues.iter().filter(|i| i.level == Level::Error).count(),
            issues.iter().filter(|i| i.level == Level::Warning).count()
        );
        for issue in &issues {
            println!("  {issue}");
        }
        failed |= issues.iter().any(|i| i.level == Level::Error);
        for view in &plugin.manifest.views {
            let svg = with_hotspots(&plugin, &view.id, &themed(&plugin.svgs[&view.id], &palette));
            let background = if light { [245, 246, 248] } else { [14, 15, 17] };
            let path = out.join(format!("{}-{}.png", plugin.id(), view.id));
            match render(&svg, background) {
                Ok(pixmap) => {
                    pixmap.save_png(&path).expect("write PNG");
                    println!("  {}", path.display());
                }
                Err(e) => {
                    println!("  view {}: {e}", view.id);
                    failed = true;
                }
            }
        }
    }
    std::process::exit(i32::from(failed));
}

/// The SVG with a marker and a label (id and inputs) per hotspot of the view.
fn with_hotspots(plugin: &DevicePlugin, view: &str, svg: &str) -> String {
    let mut marks = String::new();
    for control in plugin.manifest.controls.iter().filter(|c| c.view == view) {
        let [x, y] = control.at;
        let inputs: Vec<String> = control
            .parts()
            .into_iter()
            .map(|part| {
                let input = plugin
                    .input_of(&control.id, part)
                    .map_or("?".to_string(), |i| i.to_string());
                if part.is_empty() {
                    input
                } else {
                    format!("{part}={input}")
                }
            })
            .collect();
        let (dx, anchor) = match control.side {
            Side::Left => (-16.0, "end"),
            Side::Right => (16.0, "start"),
        };
        marks.push_str(&format!(
            r##"<circle cx="{x}" cy="{y}" r="9" fill="none" stroke="#c6f432" stroke-width="3"/>
<text x="{tx}" y="{y}" font-family="Segoe UI, DejaVu Sans, sans-serif" font-size="13" fill="#c6f432" text-anchor="{anchor}">{id}</text>
<text x="{tx}" y="{ty}" font-family="Segoe UI, DejaVu Sans, sans-serif" font-size="10" fill="#9fb0a0" text-anchor="{anchor}">{inputs}</text>
"##,
            tx = x + dx,
            ty = y + 13.0,
            id = control.id,
            inputs = inputs.join(" "),
        ));
    }
    match svg.rfind("</svg>") {
        Some(end) => format!("{}{marks}{}", &svg[..end], &svg[end..]),
        None => svg.to_string(),
    }
}

fn render(svg: &str, [r, g, b]: [u8; 3]) -> Result<tiny_skia::Pixmap, String> {
    let mut options = usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let tree = usvg::Tree::from_str(svg, &options).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or("empty picture")?;
    pixmap.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    Ok(pixmap)
}

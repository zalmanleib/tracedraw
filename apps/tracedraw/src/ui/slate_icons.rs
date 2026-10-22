//! The Slate icons: line icons kept as SVG path data (see
//! `slate_icons_data.rs`), drawn in software at the size and scale they
//! are shown at and cached as textures, then tinted with the icon colour.
//! They take the place of the painted tool, toolbar and menu icons.

use super::slate_icons_data::{Part, ICONS};
use egui::{Color32, Painter, Rect, TextureHandle, TextureOptions};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// The icon `key` ("tool.Pick", "action.Save", "glyph.SaveAs"), if there is one.
fn find(key: &str) -> Option<(f32, &'static [Part])> {
    let k = static_key(key)?;
    ICONS.iter().find(|e| e.0 == k).map(|e| (e.1, e.2))
}

/// True when there is an icon called `key`.
pub fn has(key: &str) -> bool {
    find(key).is_some()
}

/// The icon in white with its alpha, `side` pixels square.
pub fn raster(key: &str, side: u32) -> Option<egui::ColorImage> {
    let (view, parts) = find(key)?;
    let side = side.clamp(4, 512);
    let mut pm = tiny_skia::Pixmap::new(side, side)?;
    let k = side as f32 / view;
    let ts = tiny_skia::Transform::from_scale(k, k);
    for &(d, stroke, fill, stroke_alpha, dash_on, dash_off) in parts {
        let Some(path) = path(d) else { continue };
        if fill != 0.0 {
            let mut paint = tiny_skia::Paint::default();
            paint.anti_alias = true;
            if fill < 0.0 {
                // A knockout: the panel shows through.
                paint.blend_mode = tiny_skia::BlendMode::Clear;
                paint.set_color_rgba8(0, 0, 0, 255);
            } else {
                paint.set_color(white(fill));
            }
            pm.fill_path(&path, &paint, tiny_skia::FillRule::Winding, ts, None);
        }
        if stroke > 0.0 {
            let mut paint = tiny_skia::Paint::default();
            paint.anti_alias = true;
            paint.set_color(white(stroke_alpha));
            // Hairlines stay at least one pixel wide at small sizes.
            let width = stroke.max(0.85 / k);
            let mut st = tiny_skia::Stroke {
                width,
                line_cap: tiny_skia::LineCap::Round,
                line_join: tiny_skia::LineJoin::Round,
                ..Default::default()
            };
            if dash_on > 0.0 {
                st.dash = tiny_skia::StrokeDash::new(vec![dash_on, dash_off.max(0.1)], 0.0);
            }
            pm.stroke_path(&path, &paint, &st, ts, None);
        }
    }
    let px: Vec<Color32> = pm
        .pixels()
        .iter()
        .map(|p| Color32::from_white_alpha(p.alpha()))
        .collect();
    Some(egui::ColorImage::new([side as usize, side as usize], px))
}

fn white(alpha: f32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba(1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0))
        .unwrap_or(tiny_skia::Color::WHITE)
}

/// SVG path data as a tiny-skia path (through kurbo's parser, which
/// turns arcs into curves).
fn path(d: &str) -> Option<tiny_skia::Path> {
    let bez = kurbo::BezPath::from_svg(d).ok()?;
    let mut pb = tiny_skia::PathBuilder::new();
    for el in bez.elements() {
        match *el {
            kurbo::PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            kurbo::PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            kurbo::PathEl::QuadTo(a, p) => {
                pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32)
            }
            kurbo::PathEl::CurveTo(a, b, p) => pb.cubic_to(
                a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32,
            ),
            kurbo::PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

type Cache = HashMap<(&'static str, u32), TextureHandle>;

/// The icon's name as kept in the table.
fn static_key(key: &str) -> Option<&'static str> {
    static INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    let index = INDEX.get_or_init(|| ICONS.iter().enumerate().map(|(i, e)| (e.0, i)).collect());
    index.get(key).map(|&i| ICONS[i].0)
}

/// Draw the icon `key` filling `r` in `color`; false when there is no such icon.
pub fn draw(painter: &Painter, r: Rect, key: &str, color: Color32) -> bool {
    let Some(name) = static_key(key) else {
        return false;
    };
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point();
    let side = (r.width().min(r.height()) * ppp).round().max(4.0) as u32;
    // Textures belong to their context: the cache lives in it.
    let id = egui::Id::new("slate_icon_cache");
    let cached: Option<TextureHandle> = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(id)
            .lock()
            .ok()
            .and_then(|c| c.get(&(name, side)).cloned())
    });
    let tex = match cached {
        Some(t) => t,
        None => {
            let Some(img) = raster(name, side) else {
                return false;
            };
            let t = ctx.load_texture(
                format!("slate-icon-{name}-{side}"),
                img,
                TextureOptions::LINEAR,
            );
            ctx.data_mut(|d| {
                if let Ok(mut c) = d.get_temp_mut_or_default::<Arc<Mutex<Cache>>>(id).lock() {
                    c.insert((name, side), t.clone());
                }
            });
            t
        }
    };
    let s = r.width().min(r.height());
    let sq = Rect::from_center_size(r.center(), egui::vec2(s, s));
    painter.image(
        tex.id(),
        sq,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        color,
    );
    true
}

/// A tool's icon: the key is the tool's name.
pub fn tool(painter: &Painter, r: Rect, tool: crate::tools::Tool, color: Color32) -> bool {
    draw(painter, r, &format!("tool.{tool:?}"), color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_something() {
        for (key, _, _) in ICONS {
            let img = raster(key, 32).unwrap_or_else(|| panic!("{key} does not draw"));
            let ink = img.pixels.iter().filter(|p| p.a() > 64).count();
            assert!(ink > 20, "{key} is nearly empty ({ink} pixels)");
        }
    }

    #[test]
    fn every_tool_has_an_icon() {
        for g in crate::tools::GROUPS {
            for t in g.tools {
                assert!(has(&format!("tool.{t:?}")), "no icon for {t:?}");
            }
        }
    }
}

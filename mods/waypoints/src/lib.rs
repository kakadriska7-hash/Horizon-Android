use horizon_mod_sdk::{
    Mod, Registration, chat, game,
    render::{self, FillShape, Point, QuadShape, Rect, Shape, TextShape},
    storage,
    ui::{self, ButtonWidget, ListWidget, Widget},
};

const STORAGE_KEY: &str = "waypoints_v2";
const ENABLED_KEY: &str = "enabled_v2";
const COLOR_KEY: &str = "color_v2";

const BTN_SAVE_HERE: u32 = 1;
const LIST_WAYPOINTS: u32 = 2;
const BTN_DELETE: u32 = 3;
const BTN_TOGGLE_ALL: u32 = 4;
const BTN_COLOR: u32 = 5;

const CRYSTAL_COLORS: [(&str, u32, u32); 8] = [
    ("Green", 0xFF39FF14, 0xCC0A1A0A),
    ("Cyan", 0xFF00F0FF, 0xCC05181C),
    ("Red", 0xFFFF3333, 0xCC1F0707),
    ("Gold", 0xFFFFB800, 0xCC1F1604),
    ("Purple", 0xFFB84DFF, 0xCC160821),
    ("Pink", 0xFFFF55AA, 0xCC1F0814),
    ("Blue", 0xFF3388FF, 0xCC071021),
    ("White", 0xFFFFFFFF, 0xCC141418),
];

#[derive(Clone)]
struct Waypoint {
    name: String,
    x: i32,
    y: i32,
    z: i32,
    dimension: String,
}

struct WaypointsMod {
    points: Vec<Waypoint>,
    selected: Option<usize>,
    visible: bool,
    color_idx: usize,
}

impl WaypointsMod {
    fn load() -> Self {
        let mut points = Vec::new();
        if let Some(bytes) = storage::get(STORAGE_KEY) {
            if let Ok(text) = String::from_utf8(bytes) {
                for line in text.lines() {
                    let parts: Vec<&str> = line.split('\t').collect();
                    if parts.len() == 5 {
                        if let (Ok(x), Ok(y), Ok(z)) = (
                            parts[1].parse::<i32>(),
                            parts[2].parse::<i32>(),
                            parts[3].parse::<i32>(),
                        ) {
                            points.push(Waypoint {
                                name: parts[0].to_string(),
                                x,
                                y,
                                z,
                                dimension: parts[4].to_string(),
                            });
                        }
                    }
                }
            }
        }

        let visible = storage::get(ENABLED_KEY)
            .and_then(|b| String::from_utf8(b).ok())
            .map(|s| s.trim() != "false")
            .unwrap_or(true);

        let color_idx = storage::get(COLOR_KEY)
            .and_then(|b| String::from_utf8(b).ok())
            .and_then(|s| s.trim().parse::<usize>().ok())
            .filter(|&i| i < CRYSTAL_COLORS.len())
            .unwrap_or(0);

        let selected = if points.is_empty() { None } else { Some(0) };

        Self {
            points,
            selected,
            visible,
            color_idx,
        }
    }

    fn save(&self) {
        let mut out = String::new();
        for wp in &self.points {
            let clean_name = wp.name.replace(['\t', '\n', '\r'], " ");
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                clean_name, wp.x, wp.y, wp.z, wp.dimension
            ));
        }
        let _ = storage::set(STORAGE_KEY, out.as_bytes());
        let _ = storage::set(
            ENABLED_KEY,
            if self.visible { b"true" } else { b"false" },
        );
        let _ = storage::set(COLOR_KEY, self.color_idx.to_string().as_bytes());
    }

    fn refresh_settings_ui(&self) {
        let rows: Vec<String> = self
            .points
            .iter()
            .map(|wp| format!("{} ({}, {}, {})", wp.name, wp.x, wp.y, wp.z))
            .collect();

        ui::set_settings(&[
            Widget::Button(ButtonWidget {
                id: BTN_SAVE_HERE,
                label: "Save Waypoint Here".to_string(),
            }),
            Widget::List(ListWidget {
                id: LIST_WAYPOINTS,
                rows,
                selected: self.selected.map(|i| i as u32),
            }),
            Widget::Button(ButtonWidget {
                id: BTN_DELETE,
                label: "Delete Selected".to_string(),
            }),
            Widget::Button(ButtonWidget {
                id: BTN_COLOR,
                label: format!("Crystal Color: {}", CRYSTAL_COLORS[self.color_idx].0),
            }),
            Widget::Button(ButtonWidget {
                id: BTN_TOGGLE_ALL,
                label: format!("Show Waypoints: {}", if self.visible { "ON" } else { "OFF" }),
            }),
        ]);
    }

    fn project_to_screen(
        player: &game::Player,
        wp: &Waypoint,
        screen_w: f32,
        screen_h: f32,
    ) -> (f32, f32) {
        let eye_y = player.y + 1.62;
        let dx = (wp.x as f64 + 0.5) - player.x;
        let dy = (wp.y as f64 + 1.5) - eye_y;
        let dz = (wp.z as f64 + 0.5) - player.z;

        let yaw_rad = (player.yaw as f64).to_radians();
        let pitch_rad = (player.pitch as f64).to_radians();

        let fx = -yaw_rad.sin() * pitch_rad.cos();
        let fy = -pitch_rad.sin();
        let fz = yaw_rad.cos() * pitch_rad.cos();

        let rx = -fz;
        let rz = fx;
        let r_len = (rx * rx + rz * rz).sqrt().max(1e-6);
        let rx = rx / r_len;
        let rz = rz / r_len;

        let ux = -rz * fy;
        let uy = rz * fx - rx * fz;
        let uz = rx * fy;

        let cam_x = dx * rx + dz * rz;
        let cam_y = dx * ux + dy * uy + dz * uz;
        let cam_z = dx * fx + dy * fy + dz * fz;

        let fov_scale = 1.0 / (35.0_f64.to_radians().tan());
        let half_w = screen_w as f64 * 0.5;
        let half_h = screen_h as f64 * 0.5;

        let (mut sx, mut sy) = if cam_z > 0.1 {
            let ndc_x = (cam_x / cam_z) * fov_scale * (half_h / half_w);
            let ndc_y = (cam_y / cam_z) * fov_scale;
            (half_w + ndc_x * half_w, half_h - ndc_y * half_h)
        } else {
            let len = (cam_x * cam_x + cam_y * cam_y).sqrt().max(1e-6);
            let dir_x = cam_x / len;
            let dir_y = cam_y / len;
            (half_w + dir_x * half_w * 2.0, half_h - dir_y * half_h * 2.0)
        };

        let pad_x = 34.0;
        let pad_top = 34.0;
        let pad_bottom = 24.0;
        sx = sx.clamp(pad_x, (screen_w as f64 - pad_x).max(pad_x));
        sy = sy.clamp(pad_top, (screen_h as f64 - pad_bottom).max(pad_top));

        (sx as f32, sy as f32)
    }

    fn render_waypoints(&self) {
        if !self.visible || self.points.is_empty() {
            render::set_hud(&[]);
            return;
        }

        let Some(player) = game::get_player() else {
            render::set_hud(&[]);
            return;
        };

        let (screen_w, screen_h) = render::gui_size();
        if screen_w <= 0.0 || screen_h <= 0.0 {
            return;
        }

        let (_, crystal_fill, crystal_border) = CRYSTAL_COLORS[self.color_idx];
        let mut shapes: Vec<Shape> = Vec::with_capacity(self.points.len() * 5);

        for wp in &self.points {
            if wp.dimension != player.dimension {
                continue;
            }

            let dx = (wp.x as f64 + 0.5) - player.x;
            let dy = (wp.y as f64) - player.y;
            let dz = (wp.z as f64 + 0.5) - player.z;
            let dist = (dx * dx + dy * dy + dz * dz).sqrt().round() as i32;

            let (cx, cy) = Self::project_to_screen(&player, wp, screen_w, screen_h);

            let outer_rx = 6.0;
            let outer_ry = 9.0;
            shapes.push(Shape::Quad(QuadShape {
                corners: (
                    Point { x: cx, y: cy - outer_ry },
                    Point { x: cx + outer_rx, y: cy },
                    Point { x: cx, y: cy + outer_ry },
                    Point { x: cx - outer_rx, y: cy },
                ),
                color: crystal_border,
            }));

            let inner_rx = 4.5;
            let inner_ry = 7.0;
            shapes.push(Shape::Quad(QuadShape {
                corners: (
                    Point { x: cx, y: cy - inner_ry },
                    Point { x: cx + inner_rx, y: cy },
                    Point { x: cx, y: cy + inner_ry },
                    Point { x: cx - inner_rx, y: cy },
                ),
                color: crystal_fill,
            }));

            let dist_str = format!("{}m", dist);
            let name_w = render::text_width(&wp.name);
            let dist_w = render::text_width(&dist_str);
            let box_w = name_w.max(dist_w) + 8.0;
            let box_h = 20.0;
            let box_x = cx - box_w * 0.5;
            let box_y = cy - outer_ry - box_h - 3.0;

            shapes.push(Shape::Fill(FillShape {
                rect: Rect {
                    x: box_x,
                    y: box_y,
                    w: box_w,
                    h: box_h,
                },
                color: 0xB8141418,
            }));

            shapes.push(Shape::Text(TextShape {
                text: wp.name.clone(),
                at: Point {
                    x: cx,
                    y: box_y + 2.0,
                },
                color: 0xFFFFFF,
                centered: true,
                shadow: true,
            }));

            shapes.push(Shape::Text(TextShape {
                text: dist_str,
                at: Point {
                    x: cx,
                    y: box_y + 11.0,
                },
                color: 0xCCCCCC,
                centered: true,
                shadow: true,
            }));
        }

        render::set_hud(&shapes);
    }

    fn add_current(&mut self, custom_name: Option<String>) -> Result<String, String> {
        let player = game::get_player().ok_or("You must be in a world")?;
        let name = custom_name
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("Point #{}", self.points.len() + 1));

        let x = player.x.floor() as i32;
        let y = player.y.floor() as i32;
        let z = player.z.floor() as i32;

        if let Some(pos) = self.points.iter().position(|p| p.name.eq_ignore_ascii_case(&name)) {
            self.points[pos] = Waypoint {
let idx = row as usize;
if idx < self.points.len() {
self.selected = Some(idx);
}
}
BTN_DELETE => {
if let Some(idx) = self.selected {
if idx < self.points.len() {
self.points.remove(idx);
self.selected = if self.points.is_empty() { None } else { Some(0) };
self.save();
self.render_waypoints();
}
}
}
BTN_COLOR => {
self.color_idx = (self.color_idx + 1) % CRYSTAL_COLORS.len();
self.save();
self.render_waypoints();
}
BTN_TOGGLE_ALL => {
self.visible = !self.visible;
self.save();
self.render_waypoints();
}
_ => {}
}
self.refresh_settings_ui();
}
}
horizon_mod_sdk::export_mod!(WaypointsMod);

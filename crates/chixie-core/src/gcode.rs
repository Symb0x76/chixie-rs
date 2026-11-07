//! `Document` → Grbl G-code for a pen plotter.
//!
//! Each stroke: travel (`G0`) to its start, pen down (+ optional dwell), write moves (`G1`), pen
//! up. The page starts with a pen-up. Feeds are always explicit: the ChiXie3 pen literals carry no
//! `F` and rely on the modal feed (see docs/protocol.md).

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::layout::{Document, Page};

/// Controller firmware family (ChiXie3 `firmware_type`). Only Grbl variants stream today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Firmware {
    /// `ChiXie`: classic Grbl board.
    Grbl,
    /// `ESP32_Grbl`.
    Esp32Grbl,
    /// `ChiXie_ESP32_Grbl` (the verified board: Grbl_ESP32 1.3a).
    #[default]
    ChiXieEsp32Grbl,
    /// `Marlin`: not supported for streaming yet.
    Marlin,
}

impl Firmware {
    pub const ALL: [Firmware; 4] = [Firmware::Grbl, Firmware::Esp32Grbl, Firmware::ChiXieEsp32Grbl, Firmware::Marlin];
    pub fn label(self) -> &'static str {
        match self {
            Firmware::Grbl => "ChiXie (Grbl)",
            Firmware::Esp32Grbl => "ESP32_Grbl",
            Firmware::ChiXieEsp32Grbl => "ChiXie_ESP32_Grbl",
            Firmware::Marlin => "Marlin（暂不支持）",
        }
    }
    pub fn supported(self) -> bool {
        self != Firmware::Marlin
    }
}

/// How the pen is lifted (ChiXie3 `DeviceType`). The writing robots all lift with an absolute Z
/// move; Grbl_ESP32 maps Z to a stepper, an RC servo or a solenoid in firmware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceType {
    /// 步进抬笔写字机 (`WritingRobot_StepperPenUp`).
    #[default]
    StepperPenUp,
    /// 舵机抬笔写字机 (`WritingRobot_SteeringEnginePenUp`).
    ServoPenUp,
    /// 电磁铁抬笔写字机 (`WritingRobot_ElectroMagnetPenUp`).
    ElectromagnetPenUp,
    /// 自定义 Z 指令: `pen_up` / `pen_down` are sent verbatim.
    CustomZ,
}

impl DeviceType {
    pub const ALL: [DeviceType; 4] = [DeviceType::StepperPenUp, DeviceType::ServoPenUp, DeviceType::ElectromagnetPenUp, DeviceType::CustomZ];
    pub fn label(self) -> &'static str {
        match self {
            DeviceType::StepperPenUp => "步进抬笔写字机",
            DeviceType::ServoPenUp => "舵机抬笔写字机",
            DeviceType::ElectromagnetPenUp => "电磁铁抬笔写字机",
            DeviceType::CustomZ => "自定义抬落笔指令",
        }
    }
}

/// Logical → machine axis mapping (ChiXie3 `x_plus` / `y_plus` …). Identity by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AxisMap {
    /// Logical X drives machine Y (and Y drives X).
    pub swap_xy: bool,
    /// Negate the (post-swap) machine X / Y.
    pub invert_x: bool,
    pub invert_y: bool,
}

impl AxisMap {
    pub fn is_identity(&self) -> bool {
        *self == AxisMap::default()
    }
    pub fn apply(&self, (x, y): (f32, f32)) -> (f32, f32) {
        let (mut x, mut y) = if self.swap_xy { (y, x) } else { (x, y) };
        if self.invert_x {
            x = -x;
        }
        if self.invert_y {
            y = -y;
        }
        (x, y)
    }
}

/// How the device moves the pen. Defaults match the verified Grbl_ESP32 board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DeviceProfile {
    /// 设备名.
    pub name: String,
    pub firmware: Firmware,
    pub device_type: DeviceType,
    /// Pen Z positions (mm) for the Z-lift device types: `G1G90 Z<up>` / `G1G90 Z<down>`.
    pub pen_up_z: f32,
    pub pen_down_z: f32,
    /// Raw pen commands, used only by `DeviceType::CustomZ`.
    pub pen_up: String,
    pub pen_down: String,
    /// Writing feed (pen down) and travel feed (pen up), mm/min. ChiXie3 uses one 写字速度 for
    /// both; the firmware clamps to its max rate ($110/$111).
    pub write_speed: f32,
    pub travel_speed: f32,
    /// 落笔后暂停 (s): dwell after each pen-down so the pen settles. 0 = none.
    pub pause_after_down_s: f32,
    pub axis_map: AxisMap,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            name: "赤写写字机".into(),
            firmware: Firmware::default(),
            device_type: DeviceType::default(),
            pen_up_z: 0.0,
            pen_down_z: 2.0,
            pen_up: "G1G90 Z0".into(),
            pen_down: "G1G90 Z2".into(),
            write_speed: 3000.0,
            travel_speed: 8000.0,
            pause_after_down_s: 0.0,
            axis_map: AxisMap::default(),
        }
    }
}

/// `2.0` → "2", `1.25` → "1.25" (keeps the historical `Z0` / `Z2` literals byte-identical).
fn num(v: f32) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl DeviceProfile {
    pub fn pen_up_cmd(&self) -> String {
        match self.device_type {
            DeviceType::CustomZ => self.pen_up.clone(),
            _ => format!("G1G90 Z{}", num(self.pen_up_z)),
        }
    }
    pub fn pen_down_cmd(&self) -> String {
        match self.device_type {
            DeviceType::CustomZ => self.pen_down.clone(),
            _ => format!("G1G90 Z{}", num(self.pen_down_z)),
        }
    }
}

/// G-code body for one page. The pen starts and ends up.
pub fn emit(page: &Page, d: &DeviceProfile) -> String {
    let (up, down) = (d.pen_up_cmd(), d.pen_down_cmd());
    let dwell = (d.pause_after_down_s > 0.0).then(|| format!("G4 P{}", num(d.pause_after_down_s)));
    let mut g = String::new();
    let _ = writeln!(g, "{up} F{:.0}", d.travel_speed);
    for stroke in &page.strokes {
        for (i, &p) in stroke.iter().enumerate() {
            let (mx, my) = if d.axis_map.is_identity() { p } else { d.axis_map.apply(p) };
            if i == 0 {
                let _ = writeln!(g, "G0 X{mx:.3} Y{my:.3}");
                let _ = writeln!(g, "{down} F{:.0}", d.travel_speed);
                if let Some(w) = &dwell {
                    let _ = writeln!(g, "{w}");
                }
            }
            let _ = writeln!(g, "G1 X{mx:.3} Y{my:.3} F{:.0}", d.write_speed);
        }
        // Unconditional (even for an empty stroke) to stay byte-identical with the original.
        let _ = writeln!(g, "{up} F{:.0}", d.travel_speed);
    }
    g
}

/// All pages back to back, for single-sheet output (CLI). Multi-page jobs should stream
/// `emit` per page with a sheet change in between.
pub fn emit_document(doc: &Document, d: &DeviceProfile) -> String {
    let mut out = String::new();
    for (i, page) in doc.pages.iter().enumerate() {
        let body = emit(page, d);
        // Pages after the first skip their leading pen-up: the previous page ended pen-up.
        out.push_str(if i == 0 { &body } else { body.split_once('\n').map_or("", |(_, rest)| rest) });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page() -> Page {
        Page { strokes: vec![vec![(10.0, 50.0), (10.0, 48.0)]], ..Default::default() }
    }

    #[test]
    fn emits_pen_cycle() {
        let out = emit(&page(), &DeviceProfile::default());
        assert_eq!(
            out,
            "G1G90 Z0 F8000\nG0 X10.000 Y50.000\nG1G90 Z2 F8000\nG1 X10.000 Y50.000 F3000\n\
             G1 X10.000 Y48.000 F3000\nG1G90 Z0 F8000\n"
        );
    }

    #[test]
    fn empty_page_is_just_pen_up() {
        assert_eq!(emit(&Page::default(), &DeviceProfile::default()), "G1G90 Z0 F8000\n");
    }

    #[test]
    fn z_heights_dwell_and_custom_commands() {
        let d = DeviceProfile { pen_up_z: 0.5, pen_down_z: 3.25, pause_after_down_s: 0.2, ..Default::default() };
        let out = emit(&page(), &d);
        assert!(out.starts_with("G1G90 Z0.5 F8000\nG0 X10.000 Y50.000\nG1G90 Z3.25 F8000\nG4 P0.2\nG1 X10.000"), "{out}");
        let c = DeviceProfile { device_type: DeviceType::CustomZ, pen_up: "M5".into(), pen_down: "M3 S90".into(), ..Default::default() };
        assert_eq!((c.pen_up_cmd(), c.pen_down_cmd()), ("M5".to_string(), "M3 S90".to_string()));
    }

    #[test]
    fn axis_map_swaps_and_inverts() {
        let d = DeviceProfile { axis_map: AxisMap { swap_xy: true, invert_x: true, invert_y: false }, ..Default::default() };
        let out = emit(&page(), &d);
        assert!(out.contains("G0 X-50.000 Y10.000"), "{out}");
    }

    #[test]
    fn old_settings_without_new_fields_still_load() {
        let old = "pen_up = \"G1G90 Z0\"\npen_down = \"G1G90 Z2\"\nwrite_speed = 2500.0\ntravel_speed = 8000.0\n";
        let d: DeviceProfile = toml::from_str(old).unwrap();
        assert_eq!(d.write_speed, 2500.0);
        assert_eq!((d.device_type, d.pen_down_cmd()), (DeviceType::StepperPenUp, "G1G90 Z2".to_string()));
    }
}

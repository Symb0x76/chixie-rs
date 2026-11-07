//! Import ChiXie3 device parameters (`config.ini` `[DeviceConfig] DeviceParams`).
//!
//! QSettings stores the value as `@Variant(<bytes>)`: a QDataStream-serialised QVariantMap
//! `{device name: {key: value}}`, with the bytes escaped ini-style (`\0`, `\xHH`, `\b`, …). Only
//! the QVariant types ChiXie3 writes are decoded (map, string, int, bool, double).

use crate::gcode::{AxisMap, DeviceProfile, DeviceType, Firmware};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
enum Value {
    Map(Vec<(String, Value)>),
    Str(String),
    Int(i64),
    Bool(bool),
    Double(f64),
}

impl Value {
    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    fn num(&self) -> Option<f64> {
        match self {
            Value::Int(i) => Some(*i as f64),
            Value::Double(d) => Some(*d),
            Value::Str(s) => s.trim().parse().ok(),
            Value::Bool(b) => Some(f64::from(u8::from(*b))),
            _ => None,
        }
    }
    fn text(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Every device in a ChiXie3 `config.ini`, as profiles (empty if there is no `DeviceParams`).
pub fn import(ini_text: &str) -> Result<Vec<DeviceProfile>> {
    let Some(raw) = ini_text.lines().find_map(|l| l.trim().strip_prefix("DeviceParams=")) else { return Ok(Vec::new()) };
    let inner = raw.strip_prefix("@Variant(").and_then(|r| r.strip_suffix(')')).ok_or_else(|| bad("DeviceParams is not a @Variant"))?;
    let bytes = unescape(inner)?;
    let mut r = Reader { b: &bytes, i: 0 };
    let Value::Map(devices) = r.variant()? else { return Err(bad("DeviceParams is not a map")) };
    Ok(devices.iter().map(|(name, params)| profile(name, params)).collect())
}

fn profile(name: &str, p: &Value) -> DeviceProfile {
    let num = |k: &str| p.get(k).and_then(Value::num);
    let mut d = DeviceProfile { name: name.to_string(), ..Default::default() };
    if let Some(t) = num("deviceType") {
        d.device_type = device_type(t as i64);
    }
    if let Some(f) = num("firmware") {
        d.firmware = firmware(f as i64);
    }
    if let Some(v) = num("penUp") {
        d.pen_up_z = v as f32;
    }
    if let Some(v) = num("penDown") {
        d.pen_down_z = v as f32;
    }
    // ChiXie3 has one 写字速度 for every move.
    if let Some(v) = num("writeSpeed").filter(|v| *v > 0.0) {
        (d.write_speed, d.travel_speed) = (v as f32, v as f32);
    }
    if let Some(v) = num("pauseTime") {
        d.pause_after_down_s = v.max(0.0) as f32;
    }
    let dir = |k: &str| p.get(k).and_then(Value::text).unwrap_or_default().to_ascii_uppercase();
    let (xp, yp) = (dir("x_plus"), dir("y_plus"));
    let swap = xp.starts_with('Y');
    d.axis_map = AxisMap { swap_xy: swap, invert_x: xp.ends_with('-') && !swap || swap && yp.ends_with('-'), invert_y: if swap { xp.ends_with('-') } else { yp.ends_with('-') } };
    if xp.is_empty() && yp.is_empty() {
        d.axis_map = AxisMap::default();
    }
    d
}

/// ChiXie3 `DeviceType` enum order (0 = 步进抬笔写字机 confirmed on the verified board). The rest
/// follow the order of the names in the binary: **VERIFY**. Laser / 3D printer fall back to stepper.
fn device_type(i: i64) -> DeviceType {
    match i {
        1 => DeviceType::ServoPenUp,
        2 => DeviceType::ElectromagnetPenUp,
        5 => DeviceType::CustomZ,
        _ => DeviceType::StepperPenUp,
    }
}

/// ChiXie3 firmware index (3 = ChiXie_ESP32_Grbl on the verified board). Other indices: **VERIFY**.
fn firmware(i: i64) -> Firmware {
    match i {
        0 => Firmware::Grbl,
        1 => Firmware::Esp32Grbl,
        2 => Firmware::Marlin,
        _ => Firmware::ChiXieEsp32Grbl,
    }
}

fn bad(msg: &str) -> Error {
    Error::Config(format!("ChiXie3 DeviceParams: {msg}"))
}

/// Undo QSettings' ini escaping into raw bytes (each char is one Latin-1 byte).
fn unescape(s: &str) -> Result<Vec<u8>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::with_capacity(c.len());
    let mut i = 0;
    while i < c.len() {
        if c[i] != '\\' {
            out.push(u8::try_from(u32::from(c[i])).map_err(|_| bad("non-Latin-1 character"))?);
            i += 1;
            continue;
        }
        let e = *c.get(i + 1).ok_or_else(|| bad("trailing backslash"))?;
        i += 2;
        let simple = match e {
            'a' => Some(7),
            'b' => Some(8),
            'f' => Some(12),
            'n' => Some(10),
            'r' => Some(13),
            't' => Some(9),
            'v' => Some(11),
            '"' | '\'' | '?' | '\\' => Some(e as u8),
            _ => None,
        };
        if let Some(b) = simple {
            out.push(b);
        } else if e == 'x' {
            let start = i;
            while i < c.len() && c[i].is_ascii_hexdigit() && i - start < 4 {
                i += 1;
            }
            let v = u32::from_str_radix(&c[start..i].iter().collect::<String>(), 16).map_err(|_| bad("bad \\x escape"))?;
            out.push(u8::try_from(v).map_err(|_| bad("\\x escape above 0xFF"))?);
        } else if e.is_digit(8) {
            let start = i - 1;
            while i < c.len() && c[i].is_digit(8) && i - start < 3 {
                i += 1;
            }
            let v = u32::from_str_radix(&c[start..i].iter().collect::<String>(), 8).map_err(|_| bad("bad octal escape"))?;
            out.push(u8::try_from(v).map_err(|_| bad("octal escape above 0xFF"))?);
        } else {
            return Err(bad(&format!("unknown escape \\{e}")));
        }
    }
    Ok(out)
}

/// Big-endian QDataStream reader (Qt 5 format).
struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8]> {
        let end = self.i.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| bad("truncated"))?;
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().expect("4 bytes")))
    }
    fn string(&mut self) -> Result<String> {
        let n = self.u32()?;
        if n == u32::MAX {
            return Ok(String::new()); // null QString
        }
        let units: Vec<u16> = self.take(n as usize)?.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect();
        String::from_utf16(&units).map_err(|_| bad("invalid UTF-16"))
    }
    fn variant(&mut self) -> Result<Value> {
        // ChiXie3's stream has no per-variant "is null" byte: the type id is followed by the value.
        let ty = self.u32()?;
        Ok(match ty {
            1 => Value::Bool(self.u8()? != 0),
            2 => Value::Int(i64::from(self.u32()? as i32)),
            3 => Value::Int(i64::from(self.u32()?)),
            6 => Value::Double(f64::from_be_bytes(self.take(8)?.try_into().expect("8 bytes"))),
            8 => {
                let n = self.u32()?;
                let mut m = Vec::new();
                for _ in 0..n {
                    let k = self.string()?;
                    m.push((k, self.variant()?));
                }
                Value::Map(m)
            }
            10 => Value::Str(self.string()?),
            // Unknown payload size: stop rather than misread the rest.
            other => return Err(bad(&format!("unsupported QVariant type {other}"))),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `DeviceParams` line exactly as ChiXie3 3.0.310 wrote it (device "guangku").
    const INI: &str = r"[DeviceConfig]
AutoConnect=false
DeviceParams=@Variant(\0\0\0\b\0\0\0\x1\0\0\0\xe\0g\0u\0\x61\0n\0g\0k\0u\0\0\0\b\0\0\0\n\0\0\0\x10\0y\0_\0r\0\x65\0\x64\0u\0\x63\0\x65\0\0\0\n\0\0\0\x4\0Y\0-\0\0\0\f\0y\0_\0p\0l\0u\0s\0\0\0\n\0\0\0\x4\0Y\0+\0\0\0\x10\0x\0_\0r\0\x65\0\x64\0u\0\x63\0\x65\0\0\0\n\0\0\0\x4\0X\0-\0\0\0\f\0x\0_\0p\0l\0u\0s\0\0\0\n\0\0\0\x4\0X\0+\0\0\0\x14\0w\0r\0i\0t\0\x65\0S\0p\0\x65\0\x65\0\x64\0\0\0\n\0\0\0\n\0\x31\0\x39\0\x30\0\x30\0\x30\0\0\0\n\0p\0\x65\0n\0U\0p\0\0\0\x2\0\0\0\0\0\0\0\xe\0p\0\x65\0n\0\x44\0o\0w\0n\0\0\0\n\0\0\0\x2\0\x32\0\0\0\x12\0p\0\x61\0u\0s\0\x65\0T\0i\0m\0\x65\0\0\0\n\0\0\0\x2\0\x30\0\0\0\x10\0\x66\0i\0r\0m\0w\0\x61\0r\0\x65\0\0\0\x2\0\0\0\x3\0\0\0\x14\0\x64\0\x65\0v\0i\0\x63\0\x65\0T\0y\0p\0\x65\0\0\0\x2\0\0\0\0)
";

    #[test]
    fn imports_the_real_device_params() {
        let ds = import(INI).unwrap();
        assert_eq!(ds.len(), 1);
        let d = &ds[0];
        assert_eq!(d.name, "guangku");
        assert_eq!((d.device_type, d.firmware), (DeviceType::StepperPenUp, Firmware::ChiXieEsp32Grbl));
        assert_eq!((d.pen_up_z, d.pen_down_z), (0.0, 2.0));
        assert_eq!((d.write_speed, d.travel_speed, d.pause_after_down_s), (19000.0, 19000.0, 0.0));
        assert!(d.axis_map.is_identity());
        assert_eq!((d.pen_up_cmd(), d.pen_down_cmd()), ("G1G90 Z0".to_string(), "G1G90 Z2".to_string()));
    }

    #[test]
    fn unescapes_qt_ini_bytes() {
        // Hex digits right after an escape are escaped too (`\x61\x62g`, never `\x61bg`), as
        // QSettings writes them, so `\x` can read every following hex digit.
        assert_eq!(unescape(r"\0\x1\b\n\f\x61\x62g\\").unwrap(), vec![0, 1, 8, 10, 12, 0x61, 0x62, b'g', b'\\']);
    }

    #[test]
    fn missing_or_broken_params() {
        assert_eq!(import("[DeviceConfig]\nAutoConnect=false\n").unwrap(), vec![]);
        assert!(import("DeviceParams=@Variant(\\0\\0)").is_err());
        assert!(import("DeviceParams=plain").is_err());
    }
}

//! Import ChiXie3 page templates (`%USERPROFILE%\ChiXie3\conf\note_template.cx` and
//! `composition_template.cx`).
//!
//! The files are Python pickles (protocol 4) of `{name: NoteTemplate | CompositionTemplate}`.
//! A tiny pickle reader covers the opcodes those files use. Class references are only recorded
//! by name; nothing is ever executed. Field meanings: docs/protocol.md (templates).

use crate::template::{Template, TemplateKind};
use crate::{Error, Result};

/// Read every template from one `.cx` file's bytes.
pub fn import(bytes: &[u8]) -> Result<Vec<Template>> {
    let mut vm = Vm::default();
    let root = vm.run(bytes)?;
    let Node::Dict(items) = &vm.nodes[root] else { return Err(bad("top level is not a dict")) };
    let mut out = Vec::new();
    for &(k, v) in items {
        let name = vm.str(k).ok_or_else(|| bad("template key is not a string"))?.to_string();
        let Node::Object { class, state } = &vm.nodes[v] else { return Err(bad("template is not an object")) };
        let f = Fields { vm: &vm, state: *state };
        let t = match class.rsplit('.').next().unwrap_or_default() {
            "NoteTemplate" => note(&name, &f)?,
            "CompositionTemplate" => composition(&name, &f)?,
            other => return Err(bad(&format!("unknown template class {other}"))),
        };
        out.push(t);
    }
    Ok(out)
}

fn note(name: &str, f: &Fields) -> Result<Template> {
    let paper = (f.num("page_width")?, f.num("page_height")?);
    let margins = (f.num("margin_left")?, f.num("margin_top")?, f.num("margin_right")?, f.num("margin_bottom")?);
    let (lo, hi) = (f.num("min_row_num")? as u32, f.num("max_row_num")? as u32);
    let dual = f.bool("enable_dual_line_mode").unwrap_or(false);
    let kind = TemplateKind::Ruled {
        rows_min: lo.min(hi),
        rows_max: hi.max(lo),
        dual_line: dual.then(|| f.num("up_row_height").unwrap_or(2.5)),
        dashed: f.bool("enable_dashed_line_partition").unwrap_or(true),
        columns: f.num("column_num").map(|n| n as u32).unwrap_or(1).max(1),
        // Equal columns only; ChiXie3 keeps per-column gaps in `column_space`.
        column_gap: f.first_num("column_space").unwrap_or(6.0),
        divider: f.bool("show_dividing_line").unwrap_or(true),
    };
    Ok(Template { name: name.into(), paper, margins, kind })
}

fn composition(name: &str, f: &Fields) -> Result<Template> {
    Ok(Template::grid(
        name,
        (f.num("page_width")?, f.num("page_height")?),
        f.num("left_margin")?,
        f.num("top_margin")?,
        f.num("cell_row_number")? as u32,
        f.num("cell_column_number")? as u32,
        (f.num("cell_width")?, f.num("cell_height")?),
        (f.num("column_spacing").unwrap_or(0.0), f.num("row_spacing").unwrap_or(0.0)),
    ))
}

fn bad(msg: &str) -> Error {
    Error::Config(format!("ChiXie3 template file: {msg}"))
}

/// An object's `__dict__` state.
struct Fields<'a> {
    vm: &'a Vm,
    state: usize,
}

impl Fields<'_> {
    fn get(&self, key: &str) -> Option<usize> {
        let Node::Dict(items) = &self.vm.nodes[self.state] else { return None };
        items.iter().find(|(k, _)| self.vm.str(*k) == Some(key)).map(|&(_, v)| v)
    }
    fn num(&self, key: &str) -> Result<f32> {
        match self.get(key).map(|i| &self.vm.nodes[i]) {
            Some(Node::Int(v)) => Ok(*v as f32),
            Some(Node::Float(v)) => Ok(*v as f32),
            _ => Err(bad(&format!("missing number {key}"))),
        }
    }
    fn bool(&self, key: &str) -> Option<bool> {
        match self.get(key).map(|i| &self.vm.nodes[i]) {
            Some(Node::Bool(b)) => Some(*b),
            Some(Node::Int(v)) => Some(*v != 0),
            _ => None,
        }
    }
    fn first_num(&self, key: &str) -> Option<f32> {
        let Some(Node::List(xs)) = self.get(key).map(|i| &self.vm.nodes[i]) else { return None };
        match xs.first().map(|&i| &self.vm.nodes[i]) {
            Some(Node::Int(v)) => Some(*v as f32),
            Some(Node::Float(v)) => Some(*v as f32),
            _ => None,
        }
    }
}

/// Pickle values live in an arena so memoized containers can be mutated in place.
#[derive(Debug)]
enum Node {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<usize>),
    Tuple(Vec<usize>),
    Dict(Vec<(usize, usize)>),
    /// A class reference (`module.Name`), never resolved.
    Class(String),
    Object { class: String, state: usize },
}

#[derive(Default)]
struct Vm {
    nodes: Vec<Node>,
    stack: Vec<usize>,
    marks: Vec<usize>,
    memo: Vec<usize>,
}

impl Vm {
    fn str(&self, i: usize) -> Option<&str> {
        match &self.nodes[i] {
            Node::Str(s) => Some(s),
            _ => None,
        }
    }

    fn push(&mut self, n: Node) {
        self.nodes.push(n);
        self.stack.push(self.nodes.len() - 1);
    }

    fn pop(&mut self) -> Result<usize> {
        self.stack.pop().ok_or_else(|| bad("stack underflow"))
    }

    fn pop_mark(&mut self) -> Result<Vec<usize>> {
        let m = self.marks.pop().ok_or_else(|| bad("no mark"))?;
        if m > self.stack.len() {
            return Err(bad("mark past stack"));
        }
        Ok(self.stack.split_off(m))
    }

    fn top(&self) -> Result<usize> {
        self.stack.last().copied().ok_or_else(|| bad("empty stack"))
    }

    fn set_items(&mut self, dict: usize, kv: &[usize]) -> Result<()> {
        let Node::Dict(items) = &mut self.nodes[dict] else { return Err(bad("SETITEM on non-dict")) };
        for &[k, v] in kv.as_chunks::<2>().0 {
            items.push((k, v));
        }
        Ok(())
    }

    fn run(&mut self, b: &[u8]) -> Result<usize> {
        let mut r = Reader { b, i: 0 };
        loop {
            let op = r.u8()?;
            match op {
                0x80 => {
                    r.take(1)?; // PROTO
                }
                0x95 => {
                    r.take(8)?; // FRAME
                }
                b'.' => return self.pop(),
                b'(' => self.marks.push(self.stack.len()),
                b'}' => self.push(Node::Dict(Vec::new())),
                b']' => self.push(Node::List(Vec::new())),
                b')' => self.push(Node::Tuple(Vec::new())),
                b'N' => self.push(Node::None),
                0x88 => self.push(Node::Bool(true)),
                0x89 => self.push(Node::Bool(false)),
                b'K' => {
                    let v = r.u8()?;
                    self.push(Node::Int(v.into()))
                }
                b'M' => {
                    let v = u16::from_le_bytes(r.array()?);
                    self.push(Node::Int(v.into()))
                }
                b'J' => {
                    let v = i32::from_le_bytes(r.array()?);
                    self.push(Node::Int(v.into()))
                }
                0x8a => {
                    // LONG1: little-endian two's complement, n bytes
                    let n = r.u8()? as usize;
                    let bytes = r.take(n)?;
                    let mut v: i64 = 0;
                    for (k, &byte) in bytes.iter().enumerate().take(8) {
                        v |= i64::from(byte) << (8 * k);
                    }
                    if n > 0 && n < 8 && bytes[n - 1] & 0x80 != 0 {
                        v -= 1i64 << (8 * n);
                    }
                    self.push(Node::Int(v))
                }
                b'G' => {
                    let v = f64::from_be_bytes(r.array()?);
                    self.push(Node::Float(v))
                }
                0x8c => {
                    let n = r.u8()? as usize;
                    let s = r.utf8(n)?;
                    self.push(Node::Str(s))
                }
                b'X' => {
                    let n = u32::from_le_bytes(r.array()?) as usize;
                    let s = r.utf8(n)?;
                    self.push(Node::Str(s))
                }
                0x8d => {
                    let n = u64::from_le_bytes(r.array()?) as usize;
                    let s = r.utf8(n)?;
                    self.push(Node::Str(s))
                }
                0x94 => {
                    let t = self.top()?;
                    self.memo.push(t)
                }
                b'q' => {
                    let k = r.u8()? as usize;
                    self.memo_put(k)?
                }
                b'r' => {
                    let k = u32::from_le_bytes(r.array()?) as usize;
                    self.memo_put(k)?
                }
                b'h' => {
                    let k = r.u8()? as usize;
                    self.memo_get(k)?
                }
                b'j' => {
                    let k = u32::from_le_bytes(r.array()?) as usize;
                    self.memo_get(k)?
                }
                0x93 => {
                    // STACK_GLOBAL
                    let name = self.pop()?;
                    let module = self.pop()?;
                    let full = format!("{}.{}", self.str(module).unwrap_or("?"), self.str(name).unwrap_or("?"));
                    self.push(Node::Class(full))
                }
                b'c' => {
                    let module = r.line()?;
                    let name = r.line()?;
                    self.push(Node::Class(format!("{module}.{name}")))
                }
                0x81 | b'R' => {
                    // NEWOBJ (cls, args) / REDUCE (callable, args): record, never call.
                    let _args = self.pop()?;
                    let cls = self.pop()?;
                    let class = match &self.nodes[cls] {
                        Node::Class(c) => c.clone(),
                        _ => return Err(bad("NEWOBJ without a class")),
                    };
                    self.nodes.push(Node::Dict(Vec::new()));
                    let state = self.nodes.len() - 1;
                    self.push(Node::Object { class, state })
                }
                b'b' => {
                    // BUILD: object state (a dict, or (dict, slots))
                    let mut st = self.pop()?;
                    if let Node::Tuple(t) = &self.nodes[st]
                        && let Some(&first) = t.first()
                    {
                        st = first;
                    }
                    let obj = self.top()?;
                    match &mut self.nodes[obj] {
                        Node::Object { state, .. } => *state = st,
                        _ => return Err(bad("BUILD on non-object")),
                    }
                }
                b's' => {
                    let v = self.pop()?;
                    let k = self.pop()?;
                    let d = self.top()?;
                    self.set_items(d, &[k, v])?
                }
                b'u' => {
                    let kv = self.pop_mark()?;
                    let d = self.top()?;
                    self.set_items(d, &kv)?
                }
                b'a' | b'e' => {
                    let items = if op == b'a' { vec![self.pop()?] } else { self.pop_mark()? };
                    let l = self.top()?;
                    match &mut self.nodes[l] {
                        Node::List(xs) => xs.extend(items),
                        _ => return Err(bad("APPEND on non-list")),
                    }
                }
                b't' => {
                    let xs = self.pop_mark()?;
                    self.push(Node::Tuple(xs))
                }
                0x85..=0x87 => {
                    let n = (op - 0x84) as usize;
                    if self.stack.len() < n {
                        return Err(bad("stack underflow"));
                    }
                    let xs = self.stack.split_off(self.stack.len() - n);
                    self.push(Node::Tuple(xs))
                }
                other => return Err(bad(&format!("unsupported pickle opcode 0x{other:02x}"))),
            }
        }
    }

    fn memo_put(&mut self, k: usize) -> Result<()> {
        let t = self.top()?;
        if self.memo.len() <= k {
            self.memo.resize(k + 1, usize::MAX);
        }
        self.memo[k] = t;
        Ok(())
    }

    fn memo_get(&mut self, k: usize) -> Result<()> {
        let i = *self.memo.get(k).filter(|&&i| i != usize::MAX).ok_or_else(|| bad("bad memo ref"))?;
        self.stack.push(i);
        Ok(())
    }
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.i.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| bad("truncated"))?;
        let s = &self.b[self.i..end];
        self.i = end;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("length checked"))
    }
    fn utf8(&mut self, n: usize) -> Result<String> {
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| bad("invalid UTF-8"))
    }
    fn line(&mut self) -> Result<String> {
        let rest = &self.b[self.i..];
        let n = rest.iter().position(|&c| c == b'\n').ok_or_else(|| bad("unterminated GLOBAL"))?;
        let s = self.utf8(n)?;
        self.i += 1;
        Ok(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
    }

    #[test]
    fn imports_note_templates() {
        let ts = import(&fixture("note_template.cx")).unwrap();
        assert_eq!(ts.len(), 2);
        assert_eq!(ts[0], Template::ruled("信纸22行", (160.0, 219.8), (1.5, 1.0, 1.5, 1.0), 22));
        let t = &ts[1];
        assert_eq!((t.name.as_str(), t.paper, t.margins), ("随机双栏", (210.0, 297.0), (20.0, 30.0, 20.0, 20.0)));
        assert_eq!(
            t.kind,
            TemplateKind::Ruled { rows_min: 20, rows_max: 26, dual_line: Some(2.5), dashed: true, columns: 2, column_gap: 8.0, divider: true }
        );
    }

    #[test]
    fn imports_composition_grid() {
        let ts = import(&fixture("composition_template.cx")).unwrap();
        assert_eq!(ts, vec![Template::grid("作文格", (190.0, 255.0), 15.0, 20.0, 20, 20, (8.0, 8.0), (0.0, 2.9))]);
    }

    /// Reads the real ChiXie3 files of this machine's user: `cargo test -- --ignored real_files`.
    #[test]
    #[ignore]
    fn real_files() {
        let dir = std::path::PathBuf::from(std::env::var_os("USERPROFILE").unwrap()).join("ChiXie3/conf");
        for f in ["note_template.cx", "composition_template.cx"] {
            let ts = import(&std::fs::read(dir.join(f)).unwrap()).unwrap();
            assert!(!ts.is_empty());
            eprintln!("{f}: {ts:#?}");
        }
    }

    #[test]
    fn rejects_garbage_without_panicking() {
        assert!(import(b"not a pickle").is_err());
        assert!(import(&[0x80, 0x04, 0x95]).is_err());
        assert!(import(b"N.").is_err()); // valid pickle, wrong shape
    }
}

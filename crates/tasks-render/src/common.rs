use std::{
    fs::{self, File},
    io::{self, Read},
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::{DateTime, NaiveDate, Utc};
use primitive_app::{with_storage, HEIGHT, WIDTH};
use primitive_ui::{
    keyboard_key, screen_rect, Canvas, Editor, Layout, Press, Rect, TextStyle, BODY, CAPTION,
};
use serde::{de::DeserializeOwned, Serialize};

const STATE_LIMIT: usize = 2 * 1024 * 1024;

pub fn read_bounded(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut data)?;
    if data.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The file is too large.",
        ));
    }
    Ok(data)
}

pub struct State<T> {
    pub data: T,
    path: Option<PathBuf>,
    pub error: Option<String>,
}

impl<T: Default + Serialize + DeserializeOwned> State<T> {
    pub fn open(path: PathBuf) -> Self {
        let mut local = Self {
            data: T::default(),
            path: Some(path),
            error: None,
        };
        if let Err(error) = local.reload() {
            local.error = Some(format!("Could not read saved data. {error}"));
        }
        local
    }

    pub fn preview(data: T) -> Self {
        Self {
            data,
            path: None,
            error: None,
        }
    }

    pub fn reload(&mut self) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let loaded = with_storage(|| match read_bounded(path, STATE_LIMIT) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(T::default()),
            Err(error) => Err(error),
        });
        match loaded {
            Ok(data) => {
                self.data = data;
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(format!("Could not read saved data. {error}"));
                Err(error)
            }
        }
    }

    pub fn replace(&mut self, next: T) -> io::Result<()> {
        if self.error.is_some() {
            return Err(io::Error::other(
                "Repair or restore the saved file before making changes.",
            ));
        }
        let encoded = serde_json::to_vec(&next)?;
        if encoded.len() > STATE_LIMIT {
            return Err(io::Error::other(
                "The saved list has reached its size limit.",
            ));
        }
        if let Some(path) = &self.path {
            primitive_app::save(path, &encoded)?;
        }
        self.data = next;
        Ok(())
    }
}

pub fn timezone_offset(timestamp: i64) -> Option<i32> {
    let zone = fs::read("/etc/localtime").ok()?;
    kl_core::tz::offset_at(&zone, timestamp)
}

pub fn local_day() -> NaiveDate {
    let now = DateTime::<Utc>::from(SystemTime::now());
    let offset = timezone_offset(now.timestamp()).unwrap_or(0);
    let local = now
        .naive_utc()
        .checked_add_signed(chrono::TimeDelta::seconds(i64::from(offset)));
    local.unwrap_or_else(|| now.naive_utc()).date()
}

pub fn rect(x: i32, y: i32, w: i32, h: i32) -> Rect {
    screen_rect(WIDTH as i32, x, y, w, h)
}

pub fn editor(value: &str, max: usize) -> Editor {
    let mut field = Editor::new(max);
    field.text = value.chars().take(max).collect();
    field.caret = field.text.len();
    field
}

pub fn press_key(editor: &mut Editor, x: f32, y: f32) -> Option<Press> {
    let layout = Layout::new(WIDTH, HEIGHT, 4, 5);
    keyboard_key(&layout, editor, "Done", x, y).map(|key| editor.press(key))
}

pub fn field(canvas: &mut Canvas, bounds: Rect, label: &str, editor: &Editor, active: bool) {
    let pressed = canvas.control_pressed(bounds, false);
    canvas.panel(
        bounds,
        16,
        if active { 4 } else { 2 },
        if pressed { 224 } else { 255 },
    );
    let text = editor.string();
    let area = bounds.inset(18);
    canvas.text(area, if text.is_empty() { label } else { &text }, BODY);
    if active {
        let before: String = editor.text.iter().take(editor.caret).collect();
        let caret = area.x + canvas.measure(&before, 36);
        if caret < area.right() - 2 {
            canvas.fill(Rect::new(caret, area.y + 9, 3, area.h - 18), 0);
        }
    }
}

pub fn row(canvas: &mut Canvas, area: Rect, title: &str, subtitle: &str) {
    let pressed = canvas.control_pressed(area, false);
    canvas.panel(area, 20, 2, if pressed { 226 } else { 255 });
    let text = area.inset(18);
    if subtitle.is_empty() {
        canvas.text(text, title, TextStyle::new(37).bold());
    } else {
        canvas.text(
            Rect::new(text.x, text.y, text.w, text.h / 2 + 7),
            title,
            TextStyle::new(37).bold(),
        );
        canvas.text(
            Rect::new(text.x, text.y + text.h / 2 + 8, text.w, text.h / 2 - 8),
            subtitle,
            CAPTION,
        );
    }
}

pub const PAGE_SIZE: usize = 7;

pub fn page_count(count: usize) -> usize {
    count.div_ceil(PAGE_SIZE).max(1)
}

pub fn page_buttons(canvas: &mut Canvas, page: usize, count: usize) {
    let pages = page_count(count);
    if pages > 1 {
        if page > 0 {
            canvas.button(rect(40, 1460, 266, 104), "Previous", false, false);
        }
        if page + 1 < pages {
            canvas.button(rect(958, 1460, 266, 104), "Next", false, false);
        }
        canvas.text(
            rect(400, 1478, 464, 70),
            &format!("Page {} of {pages}", page + 1),
            CAPTION.center(),
        );
    }
}

pub fn page_tap(page: &mut usize, count: usize, x: f32, y: f32) -> bool {
    if *page > 0 && rect(40, 1460, 266, 104).contains(x, y) {
        *page -= 1;
        true
    } else if *page + 1 < page_count(count) && rect(958, 1460, 266, 104).contains(x, y) {
        *page += 1;
        true
    } else {
        false
    }
}

pub fn page_step(page: &mut usize, count: usize, forward: bool) -> bool {
    let next = if forward {
        (*page + 1).min(page_count(count) - 1)
    } else {
        page.saturating_sub(1)
    };
    if next == *page {
        return false;
    }
    *page = next;
    true
}

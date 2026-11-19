use chrono::{Datelike, Months, NaiveDate};
use primitive_ui::{header, Canvas, CAPTION};

use crate::common::rect;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pick {
    Date(NaiveDate),
    Clear,
    Cancel,
    Moved,
    None,
}

pub struct DatePicker {
    month: NaiveDate,
    selected: Option<NaiveDate>,
    today: NaiveDate,
    clearable: bool,
}

impl DatePicker {
    pub fn new(today: NaiveDate, selected: Option<NaiveDate>, clearable: bool) -> Self {
        let date = selected.unwrap_or(today);
        let month = date.with_day(1).expect("valid month");
        Self {
            month,
            selected,
            today,
            clearable,
        }
    }

    pub fn draw(&self, canvas: &mut Canvas) {
        header(canvas, "Choose date", "Back");
        canvas.button(rect(40, 180, 240, 104), "Previous", false, false);
        canvas.text(
            rect(294, 180, 676, 104),
            &self.month.format("%B %Y").to_string(),
            primitive_ui::TextStyle::new(44).bold().center(),
        );
        canvas.button(rect(984, 180, 240, 104), "Next", false, false);
        for (index, day) in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
            .iter()
            .enumerate()
        {
            canvas.text(day_rect(index, 278, 54), day, CAPTION.center());
        }
        for day in self.days() {
            canvas.button(
                self.bounds(day),
                &day.day().to_string(),
                self.selected == Some(day) || self.selected.is_none() && day == self.today,
                false,
            );
        }
        canvas.button(rect(40, 1144, 356, 106), "Today", false, false);
        if self.clearable {
            canvas.button(rect(868, 1144, 356, 106), "No date", false, false);
        }
    }

    pub fn tap(&mut self, x: f32, y: f32) -> Pick {
        if primitive_ui::back_button(primitive_app::WIDTH as i32).contains(x, y) {
            return Pick::Cancel;
        }
        let moved = if rect(40, 180, 240, 104).contains(x, y) {
            self.month.checked_sub_months(Months::new(1))
        } else if rect(984, 180, 240, 104).contains(x, y) {
            self.month.checked_add_months(Months::new(1))
        } else {
            None
        };
        if let Some(month) = moved.filter(|month| (1..=9999).contains(&month.year())) {
            self.month = month;
            return Pick::Moved;
        }
        if rect(40, 1144, 356, 106).contains(x, y) {
            return Pick::Date(self.today);
        }
        if self.clearable && rect(868, 1144, 356, 106).contains(x, y) {
            return Pick::Clear;
        }
        self.days()
            .find(|day| self.bounds(*day).contains(x, y))
            .map_or(Pick::None, Pick::Date)
    }

    fn days(&self) -> impl Iterator<Item = NaiveDate> + '_ {
        (1..=31).map_while(|day| self.month.with_day(day))
    }

    fn bounds(&self, day: NaiveDate) -> primitive_ui::Rect {
        let index = self.month.weekday().num_days_from_monday() + day.day() - 1;
        day_rect((index % 7) as usize, 352 + (index / 7) as i32 * 122, 108)
    }
}

fn day_rect(column: usize, y: i32, height: i32) -> primitive_ui::Rect {
    rect(40 + column as i32 * 170, y, 158, height)
}

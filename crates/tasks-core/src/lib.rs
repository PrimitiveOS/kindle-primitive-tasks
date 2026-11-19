//! Offline task planning. Dates are local civil dates without time-of-day or timezone offsets.

use chrono::{Datelike, Days, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum Repeat {
    #[default]
    Never,
    Daily,
    Weekdays,
    Weekly,
}

impl Repeat {
    pub const OPTIONS: [Self; 4] = [Self::Never, Self::Daily, Self::Weekdays, Self::Weekly];

    pub fn name(self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::Daily => "Daily",
            Self::Weekdays => "Weekdays",
            Self::Weekly => "Weekly",
        }
    }

    fn next(self, due: NaiveDate, today: NaiveDate) -> Option<NaiveDate> {
        let base = due.max(today);
        match self {
            Self::Never => None,
            Self::Daily => base.checked_add_days(Days::new(1)),
            Self::Weekdays => (1..=3)
                .filter_map(|days| base.checked_add_days(Days::new(days)))
                .find(|date| date.weekday().num_days_from_monday() < 5),
            Self::Weekly => {
                let elapsed = base.signed_duration_since(due).num_days() as u64;
                due.checked_add_days(Days::new((elapsed / 7 + 1) * 7))
            }
        }
        .filter(|date| date.year() <= 9999)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub due: Option<NaiveDate>,
    pub list: Option<u64>,
    pub repeat: Repeat,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NamedList {
    pub id: u64,
    pub title: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    Today,
    Inbox,
    Upcoming,
    List(u64),
    Completed,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Tasks {
    pub tasks: Vec<Task>,
    pub lists: Vec<NamedList>,
    undo: Option<Task>,
}

impl Tasks {
    pub fn filtered(&self, filter: Filter, today: NaiveDate) -> Vec<&Task> {
        let mut rows: Vec<_> = self
            .tasks
            .iter()
            .filter(|task| {
                if task.complete {
                    return filter == Filter::Completed;
                }
                match filter {
                    Filter::Today => task.due.is_some_and(|due| due <= today),
                    Filter::Inbox => task.list.is_none(),
                    Filter::Upcoming => task.due.is_some_and(|due| due > today),
                    Filter::List(id) => task.list == Some(id),
                    Filter::Completed => false,
                }
            })
            .collect();
        rows.sort_by_key(|task| (task.due.is_none(), task.due, task.id));
        rows
    }

    pub fn task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|item| item.id == id)
    }

    pub fn list(&self, id: u64) -> Option<&NamedList> {
        self.lists.iter().find(|item| item.id == id)
    }

    pub fn save_task(
        &mut self,
        editing: Option<u64>,
        title: &str,
        due: Option<NaiveDate>,
        list: Option<u64>,
        repeat: Repeat,
    ) -> Result<(), &'static str> {
        let title = name(title)?;
        if list.is_some_and(|id| self.list(id).is_none()) {
            return Err("That list no longer exists.");
        }
        if repeat != Repeat::Never && due.is_none() {
            return Err("Choose a due date for a repeating task.");
        }
        if due.is_some_and(|date| !(1..=9999).contains(&date.year())) {
            return Err("Choose a date between years 1 and 9999.");
        }
        if let Some(id) = editing {
            let task = self
                .tasks
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or("Task no longer exists.")?;
            task.title = title;
            task.due = due;
            task.list = list;
            task.repeat = repeat;
            self.clear_undo(id);
        } else {
            if self.tasks.len() >= 2000 {
                return Err("The task limit has been reached.");
            }
            let id = self
                .tasks
                .iter()
                .map(|task| task.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("The task limit has been reached.")?;
            self.tasks.push(Task {
                id,
                title,
                due,
                list,
                repeat,
                complete: false,
            });
        }
        Ok(())
    }

    pub fn save_list(&mut self, editing: Option<u64>, title: &str) -> Result<(), &'static str> {
        let title = name(title)?;
        if self
            .lists
            .iter()
            .any(|list| Some(list.id) != editing && list.title.eq_ignore_ascii_case(&title))
        {
            return Err("A list with that name already exists.");
        }
        if let Some(id) = editing {
            let list = self
                .lists
                .iter_mut()
                .find(|item| item.id == id)
                .ok_or("List no longer exists.")?;
            list.title = title;
        } else {
            if self.lists.len() >= 100 {
                return Err("The list limit has been reached.");
            }
            let id = self
                .lists
                .iter()
                .map(|list| list.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("The list limit has been reached.")?;
            self.lists.push(NamedList { id, title });
        }
        Ok(())
    }

    pub fn complete(
        &mut self,
        id: u64,
        today: NaiveDate,
    ) -> Result<Option<NaiveDate>, &'static str> {
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or("Task no longer exists.")?;
        let next = task.due.and_then(|due| task.repeat.next(due, today));
        self.undo = Some(task.clone());
        if let Some(due) = next {
            task.due = Some(due);
        } else {
            task.complete = true;
        }
        Ok(next)
    }

    pub fn reopen(&mut self, id: u64) -> Result<(), &'static str> {
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or("Task no longer exists.")?;
        self.undo = Some(task.clone());
        task.complete = false;
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        if let Some(before) = self.undo.take() {
            if let Some(task) = self.tasks.iter_mut().find(|task| task.id == before.id) {
                *task = before;
                return true;
            }
        }
        false
    }

    pub fn can_undo(&self) -> bool {
        self.undo.is_some()
    }

    pub fn remove(&mut self, id: u64) {
        self.tasks.retain(|task| task.id != id);
        self.clear_undo(id);
    }

    fn clear_undo(&mut self, id: u64) {
        if self.undo.as_ref().is_some_and(|task| task.id == id) {
            self.undo = None;
        }
    }
}

fn name(value: &str) -> Result<String, &'static str> {
    let text = value.trim();
    if text.is_empty() || text.chars().count() > 100 || text.chars().any(char::is_control) {
        return Err("Enter a name of 1 to 100 characters.");
    }
    Ok(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> NaiveDate {
        s.parse().unwrap()
    }

    #[test]
    fn weekly_recurrence_preserves_its_day_after_skipping_missed_weeks() {
        let mut state = Tasks::default();
        let original_due = day("2028-02-22");
        state
            .save_task(None, "Study", Some(original_due), None, Repeat::Weekly)
            .unwrap();
        assert_eq!(
            state.complete(1, day("2028-03-03")),
            Ok(Some(day("2028-03-07")))
        );
        assert!(state.undo());
        assert_eq!(state.task(1).unwrap().due, Some(original_due));
    }

    #[test]
    fn weekdays_skip_weekends_and_future_completion_stays_anchored() {
        let mut state = Tasks::default();
        let friday = day("2026-11-20");
        state
            .save_task(None, "Read", Some(friday), None, Repeat::Weekdays)
            .unwrap();
        assert_eq!(
            state.complete(1, day("2026-11-19")),
            Ok(Some(day("2026-11-23")))
        );
        assert_eq!(
            Repeat::Daily.next(day("2028-02-28"), day("2028-02-28")),
            Some(day("2028-02-29"))
        );
    }

    #[test]
    fn views_distinguish_due_and_undated_tasks_and_edits_clear_undo() {
        let mut state = Tasks::default();
        let today = day("2026-11-20");
        assert!(state
            .save_task(None, "Repeat", None, None, Repeat::Daily)
            .is_err());
        state.save_list(None, "Study").unwrap();
        state
            .save_task(None, "Read", Some(today), Some(1), Repeat::Never)
            .unwrap();
        state
            .save_task(None, "File", None, None, Repeat::Never)
            .unwrap();
        assert_eq!(state.filtered(Filter::Today, today)[0].title, "Read");
        assert_eq!(state.filtered(Filter::Inbox, today)[0].title, "File");
        state.complete(1, today).unwrap();
        assert_eq!(state.filtered(Filter::Completed, today).len(), 1);
        assert!(state.undo());
        state.complete(1, today).unwrap();
        state
            .save_task(Some(1), "New name", Some(today), Some(1), Repeat::Never)
            .unwrap();
        assert!(!state.can_undo());
    }
}

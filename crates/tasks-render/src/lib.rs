mod common;
mod date_picker;

use std::{io, path::PathBuf};

use chrono::NaiveDate;
use primitive_app::{App, Change, WIDTH};
use primitive_ui::{
    back_button, header, Canvas, Editor, Input, Press, Rect, Sheet, SheetAction, TextStyle, CAPTION,
};
use tasks_core::{Filter, Repeat, Task, Tasks};

use common::{
    editor, field, local_day, page_buttons, page_count, page_step, page_tap, press_key, rect, row,
    State, PAGE_SIZE,
};
use date_picker::{DatePicker, Pick};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Today,
    Inbox,
    Upcoming,
    Lists,
    List(u64),
    Completed,
}

const TABS: [(&str, Section); 4] = [
    ("Today", Section::Today),
    ("Inbox", Section::Inbox),
    ("Upcoming", Section::Upcoming),
    ("Lists", Section::Lists),
];

struct TaskForm {
    id: Option<u64>,
    title: Editor,
    due: Option<NaiveDate>,
    list: Option<u64>,
    repeat: Repeat,
    typing: bool,
}

struct ListForm {
    id: Option<u64>,
    title: Editor,
    typing: bool,
}

enum Choice {
    Date(DatePicker),
    List(usize),
    Repeat,
    Remove,
}

pub struct View {
    state: State<Tasks>,
    today: NaiveDate,
    live: bool,
    section: Section,
    page: usize,
    task: Option<TaskForm>,
    list: Option<ListForm>,
    choice: Option<Choice>,
    notice: String,
}

impl View {
    pub fn open(path: PathBuf) -> Self {
        Self::new(State::open(path.join("tasks.json")), local_day(), true)
    }

    pub fn preview(today: NaiveDate, data: Tasks) -> Self {
        Self::new(State::preview(data), today, false)
    }

    fn new(state: State<Tasks>, today: NaiveDate, live: bool) -> Self {
        Self {
            state,
            today,
            live,
            section: Section::Today,
            page: 0,
            task: None,
            list: None,
            choice: None,
            notice: String::new(),
        }
    }

    fn notice(&self) -> &str {
        self.state.error.as_deref().unwrap_or(&self.notice)
    }

    fn tasks(&self) -> Vec<&Task> {
        let filter = match self.section {
            Section::Today => Filter::Today,
            Section::Inbox => Filter::Inbox,
            Section::Upcoming => Filter::Upcoming,
            Section::List(id) => Filter::List(id),
            Section::Completed => Filter::Completed,
            Section::Lists => return Vec::new(),
        };
        self.state.data.filtered(filter, self.today)
    }

    fn count(&self) -> usize {
        if self.section == Section::Lists {
            self.state.data.lists.len() + 1
        } else {
            self.tasks().len()
        }
    }

    fn browse(&mut self, section: Section) -> Change {
        self.section = section;
        self.page = 0;
        self.notice.clear();
        Change::Full
    }

    fn start_task(&mut self, id: Option<u64>) -> Change {
        let (title, due, list, repeat) =
            if let Some(task) = id.and_then(|id| self.state.data.task(id)) {
                (task.title.as_str(), task.due, task.list, task.repeat)
            } else {
                let due = match self.section {
                    Section::Today => Some(self.today),
                    Section::Upcoming => self.today.succ_opt(),
                    _ => None,
                };
                let list = if let Section::List(id) = self.section {
                    Some(id)
                } else {
                    None
                };
                ("", due, list, Repeat::Never)
            };
        self.task = Some(TaskForm {
            id,
            title: editor(title, 100),
            due,
            list,
            repeat,
            typing: true,
        });
        self.notice.clear();
        Change::Full
    }

    fn start_list(&mut self, id: Option<u64>) -> Change {
        let title = id
            .and_then(|id| self.state.data.list(id))
            .map_or("", |list| list.title.as_str());
        self.list = Some(ListForm {
            id,
            title: editor(title, 100),
            typing: true,
        });
        self.notice.clear();
        Change::Full
    }

    fn write(&mut self, next: Tasks, message: &str) -> bool {
        match self.state.replace(next) {
            Ok(()) => {
                self.notice = message.into();
                self.page = self.page.min(page_count(self.count()) - 1);
                true
            }
            Err(error) => {
                self.notice = format!("Could not save. {error}");
                false
            }
        }
    }

    fn save_form(&mut self) -> Change {
        let mut next = self.state.data.clone();
        let result = match (&self.task, &self.list) {
            (Some(task), _) => next.save_task(
                task.id,
                &task.title.string(),
                task.due,
                task.list,
                task.repeat,
            ),
            (_, Some(list)) => next.save_list(list.id, &list.title.string()),
            _ => return Change::None,
        };
        match result {
            Err(message) => {
                self.notice = message.into();
                Change::Partial
            }
            Ok(()) if self.write(next, "Saved.") => {
                self.task = None;
                self.list = None;
                Change::Full
            }
            Ok(()) => Change::Partial,
        }
    }

    fn check(&mut self, id: u64, completed: bool) -> Change {
        let mut state = self.state.data.clone();
        let action = if completed {
            state
                .reopen(id)
                .map(|()| "Moved back to your tasks.".into())
        } else {
            state.complete(id, self.today).map(|next| {
                next.map_or_else(
                    || "Task completed.".into(),
                    |date| format!("Next due {}.", date.format("%-d %b")),
                )
            })
        };
        match action {
            Ok(message) => {
                self.write(state, &message);
            }
            Err(message) => self.notice = message.into(),
        }
        Change::Partial
    }

    fn undo(&mut self) -> Change {
        let mut next = self.state.data.clone();
        if !next.undo() {
            return Change::None;
        }
        self.write(next, "Task restored.");
        Change::Partial
    }

    fn draw_home(&self, canvas: &mut Canvas) {
        header(canvas, "Tasks", "Done");
        let current_tab = match self.section {
            Section::List(_) | Section::Completed => Section::Lists,
            other => other,
        };
        for (index, (label, section)) in TABS.iter().enumerate() {
            canvas.button(tab(index), label, current_tab == *section, false);
        }
        let add = match self.section {
            Section::Lists => "Add list",
            Section::Completed => "All lists",
            _ => "Add task",
        };
        let heading = match self.section {
            Section::Today => self.today.format("%A, %-d %B").to_string(),
            Section::Lists => "Your lists".into(),
            Section::List(id) => self
                .state
                .data
                .list(id)
                .map_or("List", |list| &list.title)
                .into(),
            Section::Completed => "Completed".into(),
            Section::Inbox => "Unfiled tasks".into(),
            Section::Upcoming => "Future tasks".into(),
        };
        canvas.text(
            rect(
                40,
                303,
                if matches!(self.section, Section::List(_)) {
                    543
                } else {
                    811
                },
                102,
            ),
            &heading,
            TextStyle::new(47).bold(),
        );
        canvas.button(rect(878, 301, 346, 104), add, true, false);
        if matches!(self.section, Section::List(_)) {
            canvas.button(rect(601, 301, 251, 104), "Rename", false, false);
        }
        if self.section == Section::Lists {
            let names = std::iter::once("Completed")
                .chain(self.state.data.lists.iter().map(|list| list.title.as_str()));
            for (index, title) in names
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .enumerate()
            {
                row(canvas, browse_row(index), title, "");
            }
        } else {
            let tasks = self.tasks();
            for (index, task) in tasks
                .iter()
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let check = check_bounds(index);
                let pressed = canvas.control_pressed(check, false);
                canvas.panel(check.inset(28), 6, 4, if pressed { 211 } else { 255 });
                if task.complete {
                    canvas.icon(check.inset(32), CHECKMARK, 0);
                }
                let date = match task.due {
                    Some(date) if date < self.today && !task.complete => {
                        format!("Overdue · {}", date.format("%-d %b"))
                    }
                    Some(date) if date == self.today => "Today".into(),
                    Some(date) => date.format("%-d %b %Y").to_string(),
                    None => "No date".into(),
                };
                let detail = if let Some(list) = task.list.and_then(|id| self.state.data.list(id)) {
                    format!("{date} · {}", list.title)
                } else {
                    date
                };
                row(canvas, task_bounds(index), &task.title, &detail);
            }
            if tasks.is_empty() {
                canvas.text(
                    rect(44, 464, 1180, 110),
                    if self.section == Section::Today {
                        "Nothing due today."
                    } else {
                        "No tasks here yet."
                    },
                    CAPTION,
                );
            }
        }
        if self.state.data.can_undo() {
            canvas.button(rect(878, 1300, 346, 104), "Undo", false, false);
        }
        canvas.text(rect(40, 1318, 816, 80), self.notice(), CAPTION);
        page_buttons(canvas, self.page, self.count());
    }

    fn draw_form(&self, canvas: &mut Canvas) {
        if let Some(form) = &self.task {
            header(
                canvas,
                if form.id.is_some() {
                    "Edit task"
                } else {
                    "Add task"
                },
                "Back",
            );
            field(canvas, TITLE, "Task title", &form.title, form.typing);
            let due = form.due.map_or_else(
                || "No date".into(),
                |date| date.format("%-d %B %Y").to_string(),
            );
            let list = form
                .list
                .and_then(|id| self.state.data.list(id))
                .map_or("Inbox", |list| &list.title);
            row(canvas, DUE, "Date", &due);
            row(canvas, LIST, "List", list);
            row(canvas, REPEAT, "Repeat", form.repeat.name());
            canvas.button(SAVE, "Save task", true, false);
            if form.id.is_some() {
                canvas.button(REMOVE, "Remove", false, false);
            }
            if form.typing && self.choice.is_none() {
                canvas.keyboard(form.title.layer, "Done", None);
            }
        } else if let Some(form) = &self.list {
            header(
                canvas,
                if form.id.is_some() {
                    "Rename list"
                } else {
                    "Add list"
                },
                "Back",
            );
            field(canvas, TITLE, "List name", &form.title, form.typing);
            canvas.button(SAVE, "Save list", true, false);
            if form.typing {
                canvas.keyboard(form.title.layer, "Done", None);
            }
        }
        canvas.text(rect(40, 826, 1184, 108), self.notice(), CAPTION);
    }

    fn draw_list_picker(&self, canvas: &mut Canvas, page: usize) {
        header(canvas, "Choose list", "Back");
        let names = std::iter::once("Inbox")
            .chain(self.state.data.lists.iter().map(|list| list.title.as_str()));
        for (i, title) in names.skip(page * PAGE_SIZE).take(PAGE_SIZE).enumerate() {
            row(canvas, pick_row(i), title, "");
        }
        page_buttons(canvas, page, self.state.data.lists.len() + 1);
    }

    fn tap_home(&mut self, x: f32, y: f32) -> Change {
        if back_button(WIDTH as i32).contains(x, y) {
            return Change::Exit;
        }
        for (index, (_, section)) in TABS.iter().enumerate() {
            if tab(index).contains(x, y) {
                return self.browse(*section);
            }
        }
        if rect(878, 301, 346, 104).contains(x, y) {
            return match self.section {
                Section::Lists => self.start_list(None),
                Section::Completed => self.browse(Section::Lists),
                _ => self.start_task(None),
            };
        }
        if let Section::List(id) = self.section {
            if rect(601, 301, 251, 104).contains(x, y) {
                return self.start_list(Some(id));
            }
        }
        if self.state.data.can_undo() && rect(878, 1300, 346, 104).contains(x, y) {
            return self.undo();
        }
        let count = self.count();
        if page_tap(&mut self.page, count, x, y) {
            return Change::Full;
        }
        if self.section == Section::Lists {
            for index in 0..PAGE_SIZE {
                if browse_row(index).contains(x, y) {
                    let item = self.page * PAGE_SIZE + index;
                    if item == 0 {
                        return self.browse(Section::Completed);
                    }
                    if let Some(list) = self.state.data.lists.get(item - 1) {
                        return self.browse(Section::List(list.id));
                    }
                }
            }
        } else {
            let tasks = self.tasks();
            for (index, task) in tasks
                .into_iter()
                .skip(self.page * PAGE_SIZE)
                .take(PAGE_SIZE)
                .enumerate()
            {
                let (id, completed) = (task.id, task.complete);
                if check_bounds(index).contains(x, y) {
                    return self.check(id, completed);
                }
                if task_bounds(index).contains(x, y) {
                    return self.start_task(Some(id));
                }
            }
        }
        Change::None
    }

    fn tap_form(&mut self, x: f32, y: f32) -> Change {
        if back_button(WIDTH as i32).contains(x, y) {
            self.task = None;
            self.list = None;
            self.notice.clear();
            return Change::Full;
        }
        if SAVE.contains(x, y) {
            return self.save_form();
        }
        if let Some(task) = &mut self.task {
            if TITLE.contains(x, y) {
                task.typing = true;
                return Change::Partial;
            }
            let chooser = if DUE.contains(x, y) {
                Some(Choice::Date(DatePicker::new(self.today, task.due, true)))
            } else if LIST.contains(x, y) {
                Some(Choice::List(0))
            } else if REPEAT.contains(x, y) {
                Some(Choice::Repeat)
            } else if task.id.is_some() && REMOVE.contains(x, y) {
                Some(Choice::Remove)
            } else {
                None
            };
            if let Some(choice) = chooser {
                self.choice = Some(choice);
                return Change::Full;
            }
            return typing(&mut task.title, &mut task.typing, x, y);
        }
        if let Some(form) = &mut self.list {
            if TITLE.contains(x, y) {
                form.typing = true;
                return Change::Partial;
            }
            return typing(&mut form.title, &mut form.typing, x, y);
        }
        Change::None
    }

    fn tap_choice(&mut self, x: f32, y: f32) -> Change {
        let confirm = if matches!(self.choice, Some(Choice::Remove)) {
            Some(remove_sheet().hit(x, y))
        } else {
            None
        };
        let Some(choice) = &mut self.choice else {
            return Change::None;
        };
        let Some(task) = &mut self.task else {
            return Change::None;
        };
        if let Some(hit) = confirm {
            let Some(hit) = hit else {
                return Change::None;
            };
            if hit == SheetAction::Confirm {
                let id = task.id.expect("removing an existing task");
                let mut next = self.state.data.clone();
                next.remove(id);
                if self.write(next, "Task removed.") {
                    self.task = None;
                }
            }
            self.choice = None;
            return Change::Full;
        }
        if back_button(WIDTH as i32).contains(x, y) {
            self.choice = None;
            return Change::Full;
        }
        match choice {
            Choice::Date(picker) => match picker.tap(x, y) {
                Pick::Date(date) => task.due = Some(date),
                Pick::Clear => task.due = None,
                Pick::Moved => return Change::Full,
                Pick::Cancel | Pick::None => return Change::None,
            },
            Choice::List(page) => {
                let count = self.state.data.lists.len() + 1;
                if page_tap(page, count, x, y) {
                    return Change::Full;
                }
                let choice = (0..PAGE_SIZE).find(|i| pick_row(*i).contains(x, y));
                let Some(index) = choice.map(|i| *page * PAGE_SIZE + i).filter(|i| *i < count)
                else {
                    return Change::None;
                };
                task.list = index.checked_sub(1).map(|i| self.state.data.lists[i].id);
            }
            Choice::Repeat => {
                let Some((_, repeat)) = Repeat::OPTIONS
                    .iter()
                    .enumerate()
                    .find(|(index, _)| pick_row(*index).contains(x, y))
                else {
                    return Change::None;
                };
                task.repeat = *repeat;
            }
            Choice::Remove => return Change::None,
        }
        self.choice = None;
        Change::Full
    }

    fn turn(&mut self, forward: bool) -> Change {
        if let Some(Choice::List(page)) = &mut self.choice {
            if page_step(page, self.state.data.lists.len() + 1, forward) {
                return Change::Full;
            }
        } else if self.task.is_none() && self.list.is_none() {
            let count = self.count();
            if page_step(&mut self.page, count, forward) {
                return Change::Full;
            }
        }
        Change::None
    }
}

impl App for View {
    fn draw(&self, canvas: &mut Canvas) {
        match &self.choice {
            Some(Choice::Date(picker)) => picker.draw(canvas),
            Some(Choice::List(page)) => self.draw_list_picker(canvas, *page),
            Some(Choice::Repeat) => {
                header(canvas, "Repeat", "Back");
                for (index, repeat) in Repeat::OPTIONS.iter().enumerate() {
                    row(canvas, pick_row(index), repeat.name(), "");
                }
            }
            Some(Choice::Remove) => {
                self.draw_form(canvas);
                remove_sheet().draw(canvas, None);
            }
            None if self.task.is_some() || self.list.is_some() => self.draw_form(canvas),
            None => self.draw_home(canvas),
        }
    }

    fn input(&mut self, input: Input) -> Change {
        if self.live && !matches!(input, Input::Sleep) {
            let today = local_day();
            if today != self.today {
                self.today = today;
                self.page = 0;
                return Change::Full;
            }
        }
        match input {
            Input::Tap(x, y) if self.choice.is_some() => self.tap_choice(x, y),
            Input::Tap(x, y) if self.task.is_some() || self.list.is_some() => self.tap_form(x, y),
            Input::Tap(x, y) => self.tap_home(x, y),
            Input::Swipe { dx, dy, .. } if dx.abs() > dy.abs() => self.turn(dx < 0.0),
            Input::PageForward => self.turn(true),
            Input::PageBack => self.turn(false),
            _ => Change::None,
        }
    }

    fn storage_returned(&mut self) -> io::Result<()> {
        self.state.reload()?;
        self.section = Section::Today;
        self.page = 0;
        self.task = None;
        self.list = None;
        self.choice = None;
        self.notice.clear();
        Ok(())
    }
}

fn typing(editor: &mut Editor, active: &mut bool, x: f32, y: f32) -> Change {
    if !*active {
        return Change::None;
    }
    match press_key(editor, x, y) {
        Some(Press::Go) => {
            *active = false;
            Change::Full
        }
        Some(Press::Nothing) | None => Change::None,
        _ => Change::Partial,
    }
}

const CHECKMARK: primitive_ui::Icon = primitive_ui::Icon {
    stroke: &["M5 12 L10 17 L20 7"],
    fill: &[],
};

const TITLE: Rect = Rect::new(40, 175, 1184, 104);
const DUE: Rect = Rect::new(40, 300, 1184, 106);
const LIST: Rect = Rect::new(40, 426, 1184, 106);
const REPEAT: Rect = Rect::new(40, 552, 1184, 106);
const SAVE: Rect = Rect::new(836, 694, 388, 104);
const REMOVE: Rect = Rect::new(40, 694, 290, 104);

fn tab(index: usize) -> Rect {
    rect(40 + index as i32 * 300, 174, 284, 104)
}
fn browse_row(index: usize) -> Rect {
    rect(40, 438 + index as i32 * 121, 1184, 108)
}
fn pick_row(index: usize) -> Rect {
    rect(40, 204 + index as i32 * 121, 1184, 108)
}
fn check_bounds(index: usize) -> Rect {
    rect(40, 438 + index as i32 * 121, 112, 108)
}
fn task_bounds(index: usize) -> Rect {
    rect(166, 438 + index as i32 * 121, 1058, 108)
}

fn remove_sheet() -> Sheet<'static> {
    Sheet::centered(
        rect(0, 0, 1264, 1680),
        "Remove task?",
        "This removes the task from your lists.",
        "Remove",
    )
}

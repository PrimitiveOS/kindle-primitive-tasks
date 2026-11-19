use std::{
    fs,
    path::{Path, PathBuf},
};

use primitive_app::{font_paths, frame, write_png, App, Change, HEIGHT, WIDTH};
use primitive_ui::{Canvas, Input};
use tasks_core::{Repeat, Tasks};
use tasks_render::View;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("build/previews"), PathBuf::from);
    fs::create_dir_all(&output)?;
    let fonts =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../doannt-kindle-launcher/assets/fonts");
    let mut canvas = Canvas::new(WIDTH, HEIGHT, &font_paths(&fonts))?;
    let mut capture = |view: &View, name: &str, dark| {
        write_png(
            &output.join(name),
            WIDTH,
            HEIGHT,
            &frame(view, &mut canvas, dark),
        )
    };
    let today: chrono::NaiveDate = "2026-11-20".parse()?;

    let mut model = Tasks::default();
    model.save_list(None, "Study").unwrap();
    model.save_list(None, "Home").unwrap();
    model
        .save_task(
            None,
            "Review lesson notes",
            today.pred_opt(),
            Some(1),
            Repeat::Never,
        )
        .unwrap();
    model
        .save_task(None, "Read ten pages", Some(today), Some(1), Repeat::Daily)
        .unwrap();
    model
        .save_task(
            None,
            "Check the reading list",
            Some(today),
            None,
            Repeat::Never,
        )
        .unwrap();
    model
        .save_task(
            None,
            "Prepare the next lesson",
            today.succ_opt(),
            Some(1),
            Repeat::Weekly,
        )
        .unwrap();
    model
        .save_task(None, "Organize desk notes", None, Some(2), Repeat::Never)
        .unwrap();
    let mut view = View::preview(today, model);
    capture(&view, "tasks-today.png", false)?;
    assert_eq!(view.input(Input::Tap(91.0, 491.0)), Change::Partial);
    assert_eq!(view.input(Input::Tap(1014.0, 1350.0)), Change::Partial);
    assert_eq!(view.input(Input::Tap(1040.0, 338.0)), Change::Full);
    capture(&view, "tasks-add.png", false)?;
    assert_eq!(view.input(Input::Tap(1160.0, 320.0)), Change::Full);
    capture(&view, "tasks-date.png", false)?;
    assert_eq!(view.input(Input::Tap(1100.0, 80.0)), Change::Full);
    assert_eq!(view.input(Input::Tap(1100.0, 80.0)), Change::Full);
    assert_eq!(view.input(Input::Tap(745.0, 223.0)), Change::Full);
    capture(&view, "tasks-upcoming-dark.png", true)?;
    assert_eq!(view.input(Input::Tap(1100.0, 80.0)), Change::Exit);
    Ok(())
}

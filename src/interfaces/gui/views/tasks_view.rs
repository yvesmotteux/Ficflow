use chrono::{DateTime, Utc};
use egui::{Align, Layout, RichText, ScrollArea, Ui};

use super::super::tasks::{TaskExecutor, TaskKind, TaskState, TaskStatus};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaskFilter {
    All,
    Running,
    Completed,
    Failed,
}

impl Default for TaskFilter {
    fn default() -> Self {
        Self::All
    }
}

pub struct TasksViewState<'a> {
    pub executor: &'a TaskExecutor,
    pub filter: &'a mut TaskFilter,
}

pub fn draw(ui: &mut Ui, state: TasksViewState<'_>) {
    let TasksViewState { executor, filter } = state;
    let tasks = executor.snapshot();

    ui.horizontal(|ui| {
        filter_tab(ui, filter, TaskFilter::All, "All", tasks.len());
        filter_tab(
            ui,
            filter,
            TaskFilter::Running,
            "Running",
            tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Running))
                .count(),
        );
        filter_tab(
            ui,
            filter,
            TaskFilter::Completed,
            "Completed",
            tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Done))
                .count(),
        );
        filter_tab(
            ui,
            filter,
            TaskFilter::Failed,
            "Failed",
            tasks
                .iter()
                .filter(|t| matches!(t.status, TaskStatus::Failed(_)))
                .count(),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.button("Clear completed").clicked() {
                executor.clear_completed();
            }
        });
    });
    ui.separator();

    let visible: Vec<&TaskState> = tasks
        .iter()
        .filter(|t| matches_filter(t, *filter))
        .collect();

    if visible.is_empty() {
        ui.add_space(8.0);
        ui.label(RichText::new("No tasks to show.").italics().weak());
        return;
    }

    let now = Utc::now();
    ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for task in &visible {
                // Header row: kind, display, then age + status (and Retry on failed)
                // pinned to the right. Long status/error text used to grow leftward
                // and overlap the display text — error messages now live on the
                // line below where they can wrap freely.
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format_kind(&task.kind)).strong());
                    ui.separator();
                    ui.add(egui::Label::new(&task.display).truncate().selectable(false));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(RichText::new(format_age(task.started_at, now)).weak());
                        match &task.status {
                            TaskStatus::Running => {
                                ui.spinner();
                                ui.label("Running");
                            }
                            TaskStatus::Done => {
                                ui.label(
                                    RichText::new("Done")
                                        .color(egui::Color32::from_rgb(120, 200, 120)),
                                );
                            }
                            TaskStatus::Failed(_) => {
                                if ui.button("Retry").clicked() {
                                    executor.retry(task.id);
                                }
                                ui.label(
                                    RichText::new("Failed")
                                        .color(egui::Color32::from_rgb(220, 100, 100)),
                                );
                            }
                        }
                    });
                });
                // Second line, only for failed tasks: the actual error message,
                // wrapped to the panel width.
                if let TaskStatus::Failed(msg) = &task.status {
                    ui.add(
                        egui::Label::new(
                            RichText::new(msg).color(egui::Color32::from_rgb(220, 100, 100)),
                        )
                        .wrap()
                        .selectable(true),
                    );
                }
                ui.separator();
            }
        });
}

fn filter_tab(
    ui: &mut Ui,
    current: &mut TaskFilter,
    target: TaskFilter,
    label: &str,
    count: usize,
) {
    let label = format!("{} ({})", label, count);
    if ui.selectable_label(*current == target, label).clicked() {
        *current = target;
    }
}

fn matches_filter(task: &TaskState, filter: TaskFilter) -> bool {
    match filter {
        TaskFilter::All => true,
        TaskFilter::Running => matches!(task.status, TaskStatus::Running),
        TaskFilter::Completed => matches!(task.status, TaskStatus::Done),
        TaskFilter::Failed => matches!(task.status, TaskStatus::Failed(_)),
    }
}

fn format_kind(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::Add => "Add",
    }
}

fn format_age(started: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let elapsed = now - started;
    let secs = elapsed.num_seconds();
    if secs < 5 {
        "just now".to_string()
    } else if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}

use std::env;
use std::io::{self, Write};
use std::process::ExitCode;

use super::command::{CliCommand, RatingArg, ShelfCommand};
use super::views::{details_view, list_view, shelf_list_view};
use crate::{
    application::{
        add_fic::add_fanfiction, add_to_shelf::add_to_shelf, create_shelf::create_shelf,
        delete_fic::delete_fic, delete_shelf::delete_shelf, get_fic::get_fanfiction,
        list_fics::list_fics, list_shelf_fics::list_shelf_fics, list_shelves::list_shelves,
        move_shelf::move_shelf, pin_shelf::pin_shelf, remove_from_shelf::remove_from_shelf,
        rename_shelf::rename_shelf, unpin_shelf::unpin_shelf,
        update_chapters::update_last_chapter_read, update_note::update_personal_note,
        update_rating::update_user_rating, update_read_count::update_read_count,
        update_status::update_reading_status, wipe_db::wipe_database,
    },
    domain::{fanfiction::FanfictionFetcher, repository::Repository},
    error::FicflowError,
};

pub struct CliCommandExecutor<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    repository: &'a dyn Repository,
}

impl<'a> CliCommandExecutor<'a> {
    pub fn new(fetcher: &'a dyn FanfictionFetcher, repository: &'a dyn Repository) -> Self {
        Self {
            fetcher,
            repository,
        }
    }

    pub fn execute_command(&self, command: CliCommand) -> ExitCode {
        let repo = self.repository;
        match command {
            CliCommand::Add { fic_id } => {
                println!("Adding fanfiction with ID: {}", fic_id);
                run(
                    "adding fanfiction",
                    add_fanfiction(self.fetcher, repo, fic_id),
                    |title| println!("Successfully added: {}", title),
                )
            }
            CliCommand::Delete { fic_id } => {
                println!("Deleting fanfiction with ID: {}", fic_id);
                run("deleting fanfiction", delete_fic(repo, fic_id), |()| {})
            }
            CliCommand::Get { fic_id } => {
                println!("Getting fanfiction with ID: {}", fic_id);
                run("getting fanfiction", get_fanfiction(repo, fic_id), |fic| {
                    println!("\n{}", details_view::render_fanfiction_details(&fic))
                })
            }
            CliCommand::List => {
                println!("Listing all fanfictions");
                run("listing fanfictions", list_fics(repo), |fics| {
                    println!("{}", list_view::render_fanfiction_list(&fics))
                })
            }
            CliCommand::Wipe => self.execute_wipe(),
            CliCommand::Config(_) | CliCommand::Restore { .. } => {
                unreachable!("settings commands run before the library is opened")
            }
            CliCommand::UpdateChapter { fic_id, chapter } => {
                println!(
                    "Updating last read chapter for fanfiction ID: {} to chapter {}",
                    fic_id, chapter
                );
                run(
                    "updating last read chapter",
                    update_last_chapter_read(repo, fic_id, chapter),
                    |fic| {
                        println!(
                            "Successfully updated \"{}\" (ID: {}) to chapter {}.",
                            fic.title, fic_id, chapter
                        );
                        println!("Reading Status: {}", fic.reading_status);
                        println!("Read Count: {}", fic.read_count);
                    },
                )
            }
            CliCommand::UpdateStatus { fic_id, status } => {
                println!(
                    "Updating reading status for fanfiction ID: {} to '{}'",
                    fic_id, status
                );
                run(
                    "updating reading status",
                    update_reading_status(repo, fic_id, status),
                    |fic| {
                        println!(
                            "Successfully updated \"{}\" (ID: {}) to status: {}.",
                            fic.title, fic_id, fic.reading_status
                        )
                    },
                )
            }
            CliCommand::UpdateReadCount { fic_id, read_count } => {
                println!(
                    "Updating read count for fanfiction ID: {} to {}",
                    fic_id, read_count
                );
                run(
                    "updating read count",
                    update_read_count(repo, fic_id, read_count),
                    |fic| {
                        println!(
                            "Successfully updated \"{}\" (ID: {}) to read count: {}.",
                            fic.title, fic_id, fic.read_count
                        );
                        println!("Reading Status: {}", fic.reading_status);
                    },
                )
            }
            CliCommand::UpdateRating {
                fic_id,
                rating: RatingArg(rating),
            } => {
                println!("Updating user rating for fanfiction ID: {}", fic_id);
                run(
                    "updating user rating",
                    update_user_rating(repo, fic_id, rating),
                    |fic| {
                        let rating = fic
                            .user_rating
                            .map_or_else(|| "None".to_string(), |r| r.to_string());
                        println!(
                            "Successfully updated \"{}\" (ID: {}) to rating: {}.",
                            fic.title, fic_id, rating
                        );
                    },
                )
            }
            CliCommand::UpdateNote { fic_id, note } => self.execute_update_note(fic_id, note),
            CliCommand::Shelf(sub) => self.execute_shelf(sub),
        }
    }

    fn execute_shelf(&self, command: ShelfCommand) -> ExitCode {
        let repo = self.repository;
        match command {
            ShelfCommand::Create { name, parent } => run(
                "creating shelf",
                create_shelf(repo, &name, parent),
                |shelf| {
                    println!(
                        "Created shelf \"{}\" (id: {}). Use this id to add, remove, or show fics.",
                        shelf.name, shelf.id
                    )
                },
            ),
            ShelfCommand::Delete { shelf_id } => {
                run("deleting shelf", delete_shelf(repo, shelf_id), |()| {
                    println!("Deleted shelf {}.", shelf_id)
                })
            }
            ShelfCommand::Rename { shelf_id, new_name } => run(
                "renaming shelf",
                rename_shelf(repo, shelf_id, &new_name),
                |shelf| println!("Renamed shelf {} to \"{}\".", shelf.id, shelf.name),
            ),
            ShelfCommand::Move { shelf_id, parent } => run(
                "moving shelf",
                move_shelf(repo, shelf_id, parent),
                |shelf| match shelf.parent_shelf_id {
                    Some(parent_id) => println!("Moved shelf {} under {}.", shelf.id, parent_id),
                    None => println!("Moved shelf {} to top level.", shelf.id),
                },
            ),
            ShelfCommand::Pin { shelf_id } => {
                run("pinning shelf", pin_shelf(repo, shelf_id), |shelf| {
                    println!("Pinned shelf {} (\"{}\").", shelf.id, shelf.name)
                })
            }
            ShelfCommand::Unpin { shelf_id } => {
                run("unpinning shelf", unpin_shelf(repo, shelf_id), |shelf| {
                    println!("Unpinned shelf {} (\"{}\").", shelf.id, shelf.name)
                })
            }
            ShelfCommand::List => run("listing shelves", list_shelves(repo), |shelves| {
                println!("{}", shelf_list_view::render_shelf_list(&shelves))
            }),
            ShelfCommand::Add { fic_id, shelf_id } => run(
                "adding fanfiction to shelf",
                add_to_shelf(repo, fic_id, shelf_id),
                |()| println!("Added fanfiction {} to shelf {}.", fic_id, shelf_id),
            ),
            ShelfCommand::Remove { fic_id, shelf_id } => run(
                "removing fanfiction from shelf",
                remove_from_shelf(repo, fic_id, shelf_id),
                |()| println!("Removed fanfiction {} from shelf {}.", fic_id, shelf_id),
            ),
            ShelfCommand::Show { shelf_id } => run(
                "listing shelf contents",
                list_shelf_fics(repo, shelf_id),
                |fics| println!("{}", list_view::render_fanfiction_list(&fics)),
            ),
        }
    }

    fn execute_wipe(&self) -> ExitCode {
        println!("Preparing to wipe database...");
        if !confirm(
            "WARNING: This action will delete ALL fanfictions from the database. \
             This process CANNOT be reversed!",
        ) {
            println!("Operation cancelled.");
            return ExitCode::SUCCESS;
        }
        run("wiping database", wipe_database(self.repository), |()| {
            println!("Database wiped successfully.")
        })
    }

    fn execute_update_note(&self, fic_id: u64, note: Option<String>) -> ExitCode {
        // If removing a note, show the current one first so the user sees what's being dropped.
        if note.is_none()
            && let Ok(fic) = get_fanfiction(self.repository, fic_id)
            && let Some(current_note) = &fic.personal_note
        {
            println!(
                "Current personal note for \"{}\" (ID: {}): {}",
                fic.title, fic_id, current_note
            );
            println!("Removing personal note...");
        }

        run(
            "updating personal note",
            update_personal_note(self.repository, fic_id, note.as_deref()),
            |fic| match &fic.personal_note {
                Some(note_text) => {
                    println!(
                        "Successfully added note to \"{}\" (ID: {}).",
                        fic.title, fic_id
                    );
                    println!("Note: {}", note_text);
                }
                None => println!(
                    "Successfully removed note from \"{}\" (ID: {}).",
                    fic.title, fic_id
                ),
            },
        )
    }
}

fn run<T>(verb: &str, result: Result<T, FicflowError>, on_ok: impl FnOnce(T)) -> ExitCode {
    match result {
        Ok(value) => {
            on_ok(value);
            ExitCode::SUCCESS
        }
        Err(err) => {
            report_error(verb, &err);
            ExitCode::FAILURE
        }
    }
}

fn report_error(verb: &str, err: &FicflowError) {
    match err {
        FicflowError::NotFound { fic_id } => {
            eprintln!("Fanfiction ID {} not found in your library", fic_id);
        }
        FicflowError::AlreadyExists { fic_id } => {
            eprintln!("Fanfiction ID {} is already in your library", fic_id);
        }
        FicflowError::ShelfNotFound { shelf_id } => {
            eprintln!(
                "Shelf ID {} not found. Run `ficflow shelf list` to see available shelves.",
                shelf_id
            );
        }
        FicflowError::InvalidInput(msg) => {
            eprintln!("{}", msg);
        }
        other => {
            eprintln!("Error {}: {}", verb, other);
        }
    }
}

pub(super) fn confirm(warning: &str) -> bool {
    if env::var("FICFLOW_NON_INTERACTIVE").is_ok() {
        return true;
    }

    print!("{warning}\nAre you sure you want to continue? (y/N): ");
    if io::stdout().flush().is_err() {
        return false;
    }

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    input.trim().eq_ignore_ascii_case("y")
}

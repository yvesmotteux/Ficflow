use crate::domain::fanfiction::{DatabaseOps, FanfictionFetcher};
use crate::domain::shelf::ShelfOps;

pub trait UserInterface {
    fn run(&self);
}

pub struct InterfaceFactory<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    database: &'a dyn DatabaseOps,
    shelf_ops: &'a dyn ShelfOps,
}

impl<'a> InterfaceFactory<'a> {
    pub fn new(
        fetcher: &'a dyn FanfictionFetcher,
        database: &'a dyn DatabaseOps,
        shelf_ops: &'a dyn ShelfOps,
    ) -> Self {
        Self {
            fetcher,
            database,
            shelf_ops,
        }
    }

    pub fn create_cli_interface(&self) -> Box<dyn UserInterface + '_> {
        Box::new(CliInterface {
            fetcher: self.fetcher,
            database: self.database,
            shelf_ops: self.shelf_ops,
        })
    }
}

pub struct CliInterface<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    database: &'a dyn DatabaseOps,
    shelf_ops: &'a dyn ShelfOps,
}

impl<'a> UserInterface for CliInterface<'a> {
    fn run(&self) {
        crate::interfaces::cli::run_cli(self.fetcher, self.database, self.shelf_ops);
    }
}

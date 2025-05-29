use crate::domain::fanfiction::{DatabaseOps, FanfictionFetcher};

pub trait UserInterface {
    fn run(&self);
}

pub struct InterfaceFactory<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    database: &'a dyn DatabaseOps,
}

impl<'a> InterfaceFactory<'a> {
    pub fn new(fetcher: &'a dyn FanfictionFetcher, database: &'a dyn DatabaseOps) -> Self {
        Self { fetcher, database }
    }
    
    pub fn create_cli_interface(&self) -> Box<dyn UserInterface + '_> {
        Box::new(CliInterface {
            fetcher: self.fetcher,
            database: self.database,
        })
    }
}

pub struct CliInterface<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    database: &'a dyn DatabaseOps,
}

impl<'a> UserInterface for CliInterface<'a> {
    fn run(&self) {
        crate::interfaces::cli::run_cli(self.fetcher, self.database);
    }
}

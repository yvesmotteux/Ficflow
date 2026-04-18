use crate::domain::fanfiction::FanfictionFetcher;
use crate::domain::repository::Repository;

pub trait UserInterface {
    fn run(&self);
}

pub struct InterfaceFactory<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    repository: &'a dyn Repository,
}

impl<'a> InterfaceFactory<'a> {
    pub fn new(fetcher: &'a dyn FanfictionFetcher, repository: &'a dyn Repository) -> Self {
        Self {
            fetcher,
            repository,
        }
    }

    pub fn create_cli_interface(&self) -> Box<dyn UserInterface + '_> {
        Box::new(CliInterface {
            fetcher: self.fetcher,
            repository: self.repository,
        })
    }
}

pub struct CliInterface<'a> {
    fetcher: &'a dyn FanfictionFetcher,
    repository: &'a dyn Repository,
}

impl<'a> UserInterface for CliInterface<'a> {
    fn run(&self) {
        crate::interfaces::cli::run_cli(self.fetcher, self.repository);
    }
}

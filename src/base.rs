pub mod base {
    use std::{
        collections::HashMap,
        error, io,
        iter::Rev,
        net::AddrParseError,
        ops::{Deref, DerefMut},
        os::fd::{AsRawFd, RawFd},
        result,
        slice::Iter,
    };

    use cliparser::types::{CliParsed, CliSpec};
    use mio::event::Source;

    #[derive(Debug)]
    pub enum Error {
        Msg(String),
        IoError(io::Error),
        Unknown,
        RequireOption(String),
        ParseIntError,
        AddrParseError(AddrParseError),
    }

    pub enum DebugLevel {
        None = 0,
        Crit = 1,
        Warn = 2,
        Info = 3,
    }

    impl From<io::Error> for Error {
        fn from(value: io::Error) -> Self {
            Error::IoError(value)
        }
    }

    impl From<AddrParseError> for Error {
        fn from(value: AddrParseError) -> Self {
            Error::AddrParseError(value)
        }
    }

    impl From<i32> for DebugLevel {
        fn from(value: i32) -> Self {
            match value {
                0 => DebugLevel::None,
                1 => DebugLevel::Crit,
                2 => DebugLevel::Warn,
                3 => DebugLevel::Info,
                _ => DebugLevel::Info,
            }
        }
    }

    pub trait Step: Send + Sync + BoxedClone + AsRawFd {
        fn process_data_forward(&self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error>;
        fn process_data_backward(&self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error>;
    }

    pub trait BoxedClone {
        fn bclone(&self) -> Box<dyn Step>;
    }

    pub trait StepStatic: Copy + Clone {
        fn new(args: CliParsed, debug_level: DebugLevel) -> Result<Self, Error>;
        fn get_cmd(argument: CliSpec) -> CliSpec;
    }

    impl Copy for DebugLevel {}

    impl Clone for DebugLevel {
        fn clone(&self) -> Self {
            match self {
                DebugLevel::Info => DebugLevel::Info,
                DebugLevel::Warn => DebugLevel::Warn,
                DebugLevel::Crit => DebugLevel::Crit,
                DebugLevel::None => DebugLevel::None,
            }
        }
    }

    pub trait Entry: Sized {
        fn listen(&mut self) -> Result<(), Error>;
    }

    pub trait EntryStatic<T>
    where
        T: Entry,
    {
        fn new(args: CliParsed, pipeline: Pipeline, debug_level: DebugLevel) -> Result<T, Error>;
        fn get_cmd(argument: CliSpec) -> CliSpec;
    }

    pub struct Pipeline {
        steps: Vec<Box<dyn Step>>,
    }

    impl Pipeline {
        pub fn new() -> Self {
            Pipeline { steps: Vec::new() }
        }

        pub fn add_step(&mut self, step: Box<dyn Step>) {
            self.steps.push(step);
        }
    }

    impl Deref for Pipeline {
        type Target = [Box<dyn Step>];

        fn deref(&self) -> &Self::Target {
            &self.steps
        }
    }

    impl DerefMut for Pipeline {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.steps
        }
    }

    impl Iterator for Pipeline {
        type Item = Box<dyn Step>;

        fn next(&mut self) -> Option<Self::Item> {
            todo!()
        }
    }

    impl Clone for Pipeline {
        fn clone(&self) -> Self {
            let mut result: Pipeline = Pipeline { steps: Vec::new() };
            for step in self.iter() {
                result.steps.push(step.bclone())
            }
            result
        }
    }

    impl Pipeline {
        pub fn iter_forwad(&self) -> Iter<Box<dyn Step>> {
            self.iter()
        }
        pub fn iter_backward(&self) -> Rev<Iter<Box<dyn Step>>> {
            self[0..self.len()].iter().rev()
        }
    }

    impl AsRawFd for Pipeline {
        fn as_raw_fd(&self) -> RawFd {
            self.steps.last().unwrap().as_raw_fd()
        }
    }

    impl Source for Pipeline {
        fn register(
            &mut self,
            registry: &mio::Registry,
            token: mio::Token,
            interests: mio::Interest,
        ) -> io::Result<()> {
            registry.register(
                &mut mio::unix::SourceFd(&self.as_raw_fd()),
                token,
                interests,
            )
        }

        fn reregister(
            &mut self,
            registry: &mio::Registry,
            token: mio::Token,
            interests: mio::Interest,
        ) -> io::Result<()> {
            registry.reregister(
                &mut mio::unix::SourceFd(&self.as_raw_fd()),
                token,
                interests,
            )
        }

        fn deregister(&mut self, registry: &mio::Registry) -> io::Result<()> {
            registry.deregister(&mut mio::unix::SourceFd(&self.as_raw_fd()))
        }
    }
}

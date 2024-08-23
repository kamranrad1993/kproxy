pub mod base {
    use std::{
        collections::HashMap, error, fmt::{Display, Write}, future::IntoFuture, io, iter::Rev, net::AddrParseError, ops::{Deref, DerefMut}, os::fd::{AsRawFd, RawFd}, result, slice::{Iter, IterMut}
    };

    use cliparser::types::{CliParsed, CliSpec};
    use libc::sock_filter;
    use mio::event::Source;

    #[derive(Debug)]
    pub enum Error {
        Msg(String),
        IoError(io::Error),
        Unknown,
        RequireOption(String),
        ParseIntError,
        AddrParseError(AddrParseError),
        H2_error(h2::Error)
    }

    pub enum DebugLevel {
        None = 0,
        Crit = 1,
        Warn = 2,
        Info = 3,
    }

    pub trait Step: Send + Sync + BoxedClone + AsRawFd {
        fn process_data_forward(&mut self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error>;
        fn process_data_backward(&mut self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error>;
    }

    pub trait BoxedClone {
        fn bclone(&self) -> Box<dyn Step>;
    }

    pub trait StepStatic: Clone {
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

    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Error::Msg(m) => f.write_fmt(format_args!("{}", m)),
                Error::IoError(e) => f.write_fmt(format_args!("{}", e)),
                Error::Unknown => f.write_fmt(format_args!("{}", "Unknown Error")),
                Error::RequireOption(r) => f.write_fmt(format_args!("{}", r)),
                Error::ParseIntError => f.write_fmt(format_args!("{}", "Parse Int Error")),
                Error::AddrParseError(e) => f.write_fmt(format_args!("{}", e)),
                Error::H2_error(e) => f.write_fmt(format_args!("{}", e)),
            }
        }
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

    impl From<h2::Error> for Error {
        fn from(value: h2::Error) -> Self {
            Error::H2_error(value)
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

    impl Into<i32> for &DebugLevel {
        fn into(self) -> i32 {
            match self {
                DebugLevel::None => 0,
                DebugLevel::Crit => 1,
                DebugLevel::Warn => 2,
                DebugLevel::Info => 3,
            }
        }
    }

    impl PartialEq<i32> for DebugLevel {
        fn eq(&self, other: &i32) -> bool {
            <&DebugLevel as Into<i32>>::into(self) == other.clone()
        }
    }

    impl PartialOrd<i32> for DebugLevel {
        fn partial_cmp(&self, other: &i32) -> Option<std::cmp::Ordering> {
            // <DebugLevel as PartialOrd<i32>>::partial_cmp(&self, other)
            <&DebugLevel as Into<i32>>::into(self).partial_cmp(other)

        }
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
        pub fn iter_forwad(&mut self) -> IterMut<Box<dyn Step>> {
            self.iter_mut()
        }

        pub fn iter_backward(&mut self) -> Rev<IterMut<Box<dyn Step>>> {
            let len = self.len();
            self[0..len].iter_mut().rev()
        }

        pub fn read_pipeline(&mut self) -> Result<Vec<u8>, Error> {
            let mut buffer: Vec<u8> = vec![0u8; 0];
            for step in self.iter_backward() {
                buffer = step.process_data_backward(&mut buffer)?;
            }
            Ok(buffer)
        }

        pub fn write_pipeline(&mut self, mut buffer: Vec<u8>) -> Result<(), Error> {
            for step in self.iter_forwad() {
                buffer = step.process_data_forward(buffer.to_vec().as_mut())?;
            }
            buffer.clear();
            Ok(())
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

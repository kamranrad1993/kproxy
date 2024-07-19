pub mod base {
    use std::{
        collections::HashMap,
        io,
        iter::Rev,
        ops::{Deref, DerefMut},
        slice::Iter,
    };

    use cliparser::types::{CliParsed, CliSpec};

    pub enum Error {
        IoError(io::Error),
        Unknown,
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

    pub trait Step: Send + Sync + BoxedClone {
        fn process_data_forward(&self, data: Vec<u8>) -> Result<Vec<u8>, Error>;
        fn process_data_backward(&self, data: Vec<u8>) -> Result<Vec<u8>, Error>;
    }

    pub trait BoxedClone {
        fn bclone(&self) -> Box<dyn Step>;
    }

    pub trait StepStatic: Copy + Clone {
        fn new(args: CliParsed, debug_level: DebugLevel) -> Self;
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

    pub trait Entry : Sized{
        fn listen(&mut self) -> Result<(), Error>;
    }

    pub trait EntryStatic {
        fn new(
            args: CliParsed,
            pipeline: Pipeline,
            debug_level: DebugLevel,
        ) -> Self;
        fn get_cmd(argument: CliSpec) -> CliSpec;
    }

    pub struct Pipeline {
        pub steps: Vec<Box<dyn Step>>,
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
            self[0..self.len() - 1].iter().rev()
        }
    }
}

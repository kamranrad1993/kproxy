pub mod stdio {
    use std::{
        fmt::Display,
        io::{stdin, stdout, Read, Write},
        ops::{BitAnd, BitOr},
    };

    const FORWARD_STDOUT_OPTION: (&str, &str, &str) = (
        "forward-stdout",
        "--forward-stdout",
        "(StdioStep) print into stdout when pipeline direction is forward",
    );
    const BACKWARD_STDOUT_OPTION: (&str, &str, &str) = (
        "backward-stdout",
        "--backward-stdout",
        "(StdioStep) print into stdout when pipeline direction is backward",
    );

    use cliparser::types::{
        Argument, ArgumentHelp, ArgumentOccurrence, ArgumentValueType, CliParsed, CliSpec,
    };

    use crate::{
        base::base::DebugLevel, BoxedClone, Entry, EntryStatic, Error, Pipeline, Step, StepStatic,
    };

    pub struct StdioEntry {
        pipeline: Pipeline,
        debug_level: DebugLevel,
    }

    impl Entry for StdioEntry {
        fn listen(&mut self) -> Result<(), Error> {
            loop {
                let mut io = stdin();
                let mut available: usize = 0;
                let result: i32 = unsafe { libc::ioctl(0, libc::FIONREAD, &mut available) };

                if result == -1 {
                    let errno = std::io::Error::last_os_error();
                    return Err(Error::IoError(errno));
                } else if available > 0 {
                    let mut buf = vec![0u8; available];
                    io.read(buf.as_mut_slice())?;

                    self.handle_pipeline(buf)?;
                }
            }
        }
    }

    impl StdioEntry {
        fn handle_pipeline(&mut self, mut data: Vec<u8>) -> Result<(), Error> {
            for step in self.pipeline.iter_forwad() {
                data = step.process_data_forward(&mut data)?;
            }
            for step in self.pipeline.iter_backward() {
                data = step.process_data_backward(&mut data)?;
            }
            Ok(())
        }
    }

    impl EntryStatic<StdioEntry> for StdioEntry {
        #[allow(unused_variables)]
        fn new(
            args: CliParsed,
            pipeline: Pipeline,
            debug_level: DebugLevel,
        ) -> Result<StdioEntry, Error> {
            Ok(Self {
                pipeline,
                debug_level,
            })
        }

        fn get_cmd(argument: CliSpec) -> CliSpec {
            argument
        }
    }

    enum StdoutMode {
        None = 0x00,
        Forward = 0x01,
        Backward = 0x02,
        Both = 0x03,
    }

    impl Clone for StdoutMode {
        fn clone(&self) -> Self {
            match self {
                StdoutMode::None => StdoutMode::None,
                StdoutMode::Forward => StdoutMode::Forward,
                StdoutMode::Backward => StdoutMode::Backward,
                StdoutMode::Both => StdoutMode::Both,
            }
        }
    }

    impl Copy for StdoutMode {}

    impl PartialEq for StdoutMode {
        fn eq(&self, other: &Self) -> bool {
            core::mem::discriminant(self) == core::mem::discriminant(other)
        }
    }

    impl Display for StdoutMode {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                StdoutMode::Forward => write!(f, "PipelineDirection::Forward"),
                StdoutMode::Backward => write!(f, "PipelineDirection::Backward"),
                StdoutMode::None => write!(f, "PipelineDirection::None"),
                StdoutMode::Both => write!(f, "PipelineDirection::Both"),
            }
        }
    }

    impl BitAnd for StdoutMode {
        type Output = StdoutMode;

        fn bitand(self, rhs: Self) -> Self::Output {
            let result = (self as u8) & (rhs as u8);
            match result {
                0x00 => StdoutMode::None,
                0x01 => StdoutMode::Forward,
                0x02 => StdoutMode::Backward,
                0x03 => StdoutMode::Both,
                _ => unreachable!(),
            }
        }
    }

    impl BitOr for StdoutMode {
        type Output = StdoutMode;

        fn bitor(self, rhs: Self) -> Self::Output {
            let result = (self as u8) | (rhs as u8);
            match result {
                0x00 => StdoutMode::None,
                0x01 => StdoutMode::Forward,
                0x02 => StdoutMode::Backward,
                0x03 => StdoutMode::Both,
                _ => unreachable!(),
            }
        }
    }

    pub struct StdioStep {
        stdout_mode: StdoutMode,
        debug_level: DebugLevel,
    }

    impl Step for StdioStep {
        fn process_data_forward(&self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            let mut io = stdout();
            if self.debug_level as usize > 2 {
                io.write("\n++++++++++++++++++++++++++++++++++".as_bytes())?;
                io.write("\nstdio forward : \n".as_bytes())?;
                io.write(data.as_slice())?;
                io.write("++++++++++++++++++++++++++++++++++\n".as_bytes())?;
            }
            if self.stdout_mode & StdoutMode::Forward == StdoutMode::Forward
                && self.debug_level as usize <= 2
            {
                io.write(data.as_slice())?;
            }
            Ok(data.clone())
        }

        fn process_data_backward(&self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            let mut io = stdout();
            if self.debug_level as usize > 2 {
                io.write("\n++++++++++++++++++++++++++++++++++\n".as_bytes())?;
                io.write("\nstdio backward : \n".as_bytes())?;
                io.write(data.as_slice())?;
                io.write("++++++++++++++++++++++++++++++++++\n".as_bytes())?;
            }
            if self.stdout_mode & StdoutMode::Backward == StdoutMode::Backward
                && self.debug_level as usize <= 2
            {
                io.write(data.as_slice())?;
            }
            Ok(data.clone())
        }
    }

    impl BoxedClone for StdioStep {
        fn bclone(&self) -> Box<dyn Step> {
            Box::new(Self {
                debug_level: self.debug_level.clone(),
                stdout_mode: self.stdout_mode.clone(),
            })
        }
    }

    impl StepStatic for StdioStep {
        fn new(args: CliParsed, debug_level: DebugLevel) -> Result<Self, Error> {
            let mut stdout_mode = StdoutMode::None;
            if args.arguments.contains(FORWARD_STDOUT_OPTION.0) {
                stdout_mode = stdout_mode | StdoutMode::Forward;
            }
            if args.arguments.contains(BACKWARD_STDOUT_OPTION.0) {
                stdout_mode = stdout_mode | StdoutMode::Backward;
            }
            Ok(Self {
                stdout_mode,
                debug_level,
            })
        }

        fn get_cmd(mut argument: CliSpec) -> CliSpec {
            argument = argument.add_argument(Argument {
                name: FORWARD_STDOUT_OPTION.0.to_string(),
                key: vec![FORWARD_STDOUT_OPTION.1.to_string()],
                argument_occurrence: ArgumentOccurrence::Single,
                value_type: ArgumentValueType::None,
                default_value: None,
                help: Some(ArgumentHelp::Text(BACKWARD_STDOUT_OPTION.2.to_string())),
            });

            argument = argument.add_argument(Argument {
                name: BACKWARD_STDOUT_OPTION.0.to_string(),
                key: vec![BACKWARD_STDOUT_OPTION.1.to_string()],
                argument_occurrence: ArgumentOccurrence::Single,
                value_type: ArgumentValueType::None,
                default_value: None,
                help: Some(ArgumentHelp::Text(BACKWARD_STDOUT_OPTION.2.to_string())),
            });
            argument
        }
    }

    impl Copy for StdioStep {}

    impl Clone for StdioStep {
        fn clone(&self) -> Self {
            Self {
                debug_level: self.debug_level.clone(),
                stdout_mode: self.stdout_mode.clone(),
            }
        }
    }
}

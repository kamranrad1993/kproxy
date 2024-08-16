pub mod stdio {
    use std::{
        fmt::Display,
        io::{self, stdin, stdout, Read, Stdin, Stdout, Write},
        ops::{BitAnd, BitOr},
        os::{
            fd::{AsRawFd, RawFd},
            unix::net::UnixStream,
        },
        sync::Mutex,
        time::Duration,
    };
    extern crate lazy_static;

    use cliparser::types::{
        Argument, ArgumentHelp, ArgumentOccurrence, ArgumentValueType, CliParsed, CliSpec,
    };
    use lazy_static::lazy_static;
    use mio::{event::Source, unix::SourceFd, Events, Interest, Poll, Token};
    // use mio::{Events, Interest, Poll, Token};

    use crate::{
        base::base::DebugLevel, BoxedClone, Entry, EntryStatic, Error, Pipeline, Step, StepStatic,
        TcpStep, BUFFER_SIZE,
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

    lazy_static! {
        static ref STDOUT: Mutex<io::Stdout> = Mutex::new(io::stdout());
    }

    lazy_static! {
        static ref STDIN: Mutex<Stdin> = Mutex::new(stdin());
    }

    const STDIN_TOKEN: Token = Token(0);
    const PIPELINE_TOKEN: Token = Token(1);

    pub struct StdioEntry {
        pipeline: Pipeline,
        debug_level: DebugLevel,
        buffer_size: usize,
    }

    impl Entry for StdioEntry {
        fn listen(&mut self) -> Result<(), Error> {
            let mut poll = Poll::new()?;
            let mut events = Events::with_capacity(128);
            let fd = STDIN.lock().unwrap().as_raw_fd();
            let mut fd = SourceFd(&fd);
            poll.registry()
                .register(&mut fd, STDIN_TOKEN, Interest::READABLE)?;
            let mut connection_counter = 0;

            let mut buf = vec![0u8; self.buffer_size];
            let mut buf_fill_size = 0;
            loop {
                poll.poll(&mut events, None)?;
                for event in events.iter() {
                    match event.token() {
                        STDIN_TOKEN => {
                            buf_fill_size = STDIN.lock().unwrap().read(buf.as_mut_slice())?;
                        }
                        PIPELINE_TOKEN => {
                            if event.is_readable() {
                                let data = self.pipeline.read_pipeline()?;
                            }
                            if event.is_writable() {
                                self.pipeline.write_pipeline(buf)?;
                                buf.clear();
                            }
                        }
                        _ => unreachable!(),
                    }
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
            let buffer_size = match args.argument_values.get(BUFFER_SIZE.0) {
                Some(buffer_size) => buffer_size[0].clone(),
                None => return Err(Error::RequireOption(BUFFER_SIZE.0.to_string())),
            };

            let buffer_size = match str::parse::<usize>(buffer_size.as_str()) {
                Ok(buffer_size) => buffer_size,
                Err(e) => return Err(Error::ParseIntError),
            };

            Ok(Self {
                pipeline,
                debug_level,
                buffer_size,
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
        buffer_size: usize,
        // streams: (UnixStream, UnixStream),
    }

    impl Step for StdioStep {
        fn process_data_forward(&mut self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            if self.debug_level as usize > 2 {
                STDOUT
                    .lock()
                    .unwrap()
                    .write("\n++++++++++++++++++++++++++++++++++".as_bytes())?;
                STDOUT
                    .lock()
                    .unwrap()
                    .write("\nstdio forward : \n".as_bytes())?;
                STDOUT.lock().unwrap().write(data.as_slice())?;
                STDOUT
                    .lock()
                    .unwrap()
                    .write("++++++++++++++++++++++++++++++++++\n".as_bytes())?;
                STDOUT.lock().unwrap().flush()?;
            }
            if self.stdout_mode & StdoutMode::Forward == StdoutMode::Forward
                && self.debug_level as usize <= 2
            {
                STDOUT.lock().unwrap().write(data.as_slice())?;
                STDOUT.lock().unwrap().flush()?;
            }
            Ok(data.clone())
        }

        fn process_data_backward(&mut self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            if self.debug_level as usize > 2 {
                STDOUT
                    .lock()
                    .unwrap()
                    .write("\n++++++++++++++++++++++++++++++++++\n".as_bytes())?;
                STDOUT
                    .lock()
                    .unwrap()
                    .write("\nstdio backward : \n".as_bytes())?;
                STDOUT.lock().unwrap().write(data.as_slice())?;
                STDOUT
                    .lock()
                    .unwrap()
                    .write("++++++++++++++++++++++++++++++++++\n".as_bytes())?;
                STDOUT.lock().unwrap().flush()?;
            }
            if self.stdout_mode & StdoutMode::Backward == StdoutMode::Backward
                && self.debug_level as usize <= 2
            {
                STDOUT.lock().unwrap().write(data.as_slice())?;
                STDOUT.lock().unwrap().flush()?;
            }
            let mut read_buffer = vec![0u8; self.buffer_size];
            let read_size = STDIN.lock().unwrap().read(&mut read_buffer)?;
            Ok(read_buffer[0..read_size].to_vec())
        }
    }

    impl BoxedClone for StdioStep {
        fn bclone(&self) -> Box<dyn Step> {
            Box::new(Self {
                debug_level: self.debug_level.clone(),
                stdout_mode: self.stdout_mode.clone(),
                buffer_size: self.buffer_size,
                // streams: (
                //     self.streams.0.try_clone().unwrap(),
                //     self.streams.1.try_clone().unwrap(),
                // ),
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
            let buffer_size = match args.argument_values.get(BUFFER_SIZE.0) {
                Some(buffer_size) => buffer_size[0].clone(),
                None => return Err(Error::RequireOption(BUFFER_SIZE.0.to_string())),
            };

            let buffer_size = match str::parse::<usize>(buffer_size.as_str()) {
                Ok(buffer_size) => buffer_size,
                Err(e) => return Err(Error::ParseIntError),
            };

            let (mut stream1, stream2) = UnixStream::pair()?;
            // let fd = stream2.as_raw_fd();
            // unsafe {
            //     libc::dup2(fd, 0);
            //     libc::dup2(fd, 1);
            //     libc::close(fd);
            // }

            Ok(Self {
                stdout_mode,
                debug_level,
                buffer_size,
                // streams: (stream1, stream2),
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

    // impl Copy for StdioStep {}

    impl Clone for StdioStep {
        fn clone(&self) -> Self {
            Self {
                debug_level: self.debug_level.clone(),
                stdout_mode: self.stdout_mode.clone(),
                buffer_size: self.buffer_size,
                // streams: (
                //     self.streams.0.try_clone().unwrap(),
                //     self.streams.1.try_clone().unwrap(),
                // ),
            }
        }
    }

    impl AsRawFd for StdioStep {
        fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
            // let fd = unsafe {
            //     libc::dup2(
            //         STDIN.lock().unwrap().as_raw_fd(),
            //         STDOUT.lock().unwrap().as_raw_fd(),
            //     )
            // };
            STDIN.lock().unwrap().as_raw_fd()
            // return fd as RawFd;
            // self.streams.0.as_raw_fd()
        }
    }
}

pub mod http {
    use async_std::task::block_on;
    use cliparser::types::{
        Argument, ArgumentHelp, ArgumentOccurrence, ArgumentValueType, CliParsed, CliSpec,
    };
    use hyper::body::Bytes;
    use hyper::server::conn;
    use hyper::Request;
    use id_pool::IdPool;
    use mio::unix::SourceFd;
    // use mio::net::{TcpListener, TcpStream};
    use mio::{Events, Interest, Poll, Token};
    use std::io::{ErrorKind, Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::{result, string, vec};
    use tokio::net::{TcpListener, TcpStream};

    use h2::server::{self, SendResponse};
    use h2::RecvStream;

    use crate::{
        base::base::DebugLevel, BoxedClone, Entry, EntryStatic, Error, Pipeline, Step, StepStatic,
    };
    use crate::{create_socket_addr, MultiMap, Ref, BUFFER_SIZE};

    const HTTP_ENTRY_ADDRESS: (&str, &str, &str) = (
        "http-entry-address",
        "--http-ea",
        "(HttpEntry) Http Entry listen address",
    );
    const HTTP_ENTRY_PORT: (&str, &str, &str) = (
        "http-entry-port",
        "--http-ep",
        "(HttpEntry) Http Entry listen port",
    );

    const HTTP_STEP_ADDRESS: (&str, &str, &str) = (
        "http-step-address",
        "--http-sa",
        "(HttpEntry) Http step endpoint address",
    );
    const HTTP_STEP_PORT: (&str, &str, &str) = (
        "http-step-port",
        "--http-sp",
        "(HttpEntry) Http step endpoint port",
    );

    const SERVER_TOKEN: Token = Token(0);

    pub struct HttpEntry {
        address: String,
        port: u16,
        debug_level: DebugLevel,
        pipeline_template: Pipeline,
        connections: MultiMap<Token, Ref<TcpEntryContext>>,
        buffer_size: usize,
    }

    struct TcpEntryContext {
        connection: server::Connection<TcpStream, Bytes>,
        request: Request<RecvStream>,
        response: SendResponse<Bytes>,
        pipeline: Pipeline,
        connection_buf: Vec<u8>,
        pipeline_buf: Vec<u8>,
    }

    impl Entry for HttpEntry {
        fn listen(&mut self) -> Result<(), Error> {
            let addr = create_socket_addr(self.address.as_str(), self.port)?;
            let mut server = block_on(TcpListener::bind(addr))?;

            let mut poll = Poll::new()?;
            let mut events = Events::with_capacity(128);
            let fd = server.as_raw_fd();
            let mut fd = SourceFd(&fd);
            poll.registry()
                .register(&mut fd, SERVER_TOKEN, Interest::READABLE)?;
            let mut idpool = IdPool::new();

            loop {
                poll.poll(&mut events, None)?;
                for event in events.iter() {
                    match event.token() {
                        SERVER_TOKEN => {
                            let connection = block_on(server.accept())?;
                            let fd = connection.0.as_raw_fd();
                            let mut handshacked_connection =
                                block_on(server::handshake(connection.0)).unwrap();
                            let (request, response) =
                                block_on(handshacked_connection.accept()).unwrap()?;

                            let mut client = Ref::new(TcpEntryContext {
                                connection: handshacked_connection,
                                request: request,
                                response: response,
                                pipeline: self.pipeline_template.clone(),
                                connection_buf: vec![0u8; 0],
                                pipeline_buf: vec![0u8; 0],
                            });

                            {
                                // debug
                                if self.debug_level >= 2 {
                                    println!("new client: {}", connection.1);
                                }
                            }

                            let token = Token(idpool.request_id().unwrap());
                            let mut fd = SourceFd(&fd);
                            poll.registry()
                                .register(&mut fd, token, Interest::READABLE | Interest::WRITABLE)
                                .unwrap();

                            let pipeline_token = Token(idpool.request_id().unwrap());
                            let pfd = client.pipeline.as_raw_fd();
                            let mut pfd = SourceFd(&pfd);
                            poll.registry()
                                .register(
                                    &mut pfd,
                                    pipeline_token,
                                    Interest::READABLE | Interest::WRITABLE,
                                )
                                .unwrap();

                            self.connections.insert(token, client.clone());
                            self.connections.insert(pipeline_token, client);
                            // drop(connection);
                        }
                        other => {
                            let client = match self.connections.get_mut(&other) {
                                Some(client) => client,
                                None => {
                                    if self.debug_level > 0 {
                                        eprintln!("No client found with this token {}", other.0);
                                    }
                                    continue;
                                }
                            };

                            match other.0 % 2 {
                                0 => {
                                    // pipeline has io event
                                    if event.is_readable() {
                                        if let Err(e) = HttpEntry::read_pipeline(client) {
                                            match e {
                                                Error::IoError(e) => {
                                                    if e.kind() == ErrorKind::WouldBlock {
                                                        continue;
                                                    }
                                                }
                                                _ => {
                                                    if self.debug_level > 0 {
                                                        eprintln!(
                                                            "an error accured reading pipeline: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        if let Err(e) = HttpEntry::write_client(client) {
                                            if self.debug_level > 0 {
                                                eprintln!("an error accured writing client: {}", e);
                                            }
                                        }
                                    }
                                }
                                1 => {
                                    // client has io event
                                    if event.is_readable() {
                                        if let Err(e) =
                                            HttpEntry::read_client(self.buffer_size, client)
                                        {
                                            if self.debug_level > 0 {
                                                eprintln!("an error accured reading client: {}", e);
                                            }
                                        }

                                        if let Err(e) = HttpEntry::write_pipeline(client) {
                                            match e {
                                                Error::IoError(e) => {
                                                    if e.kind() == ErrorKind::WouldBlock {
                                                        continue;
                                                    }
                                                }
                                                _ => {
                                                    if self.debug_level > 0 {
                                                        eprintln!(
                                                            "an error accured writing pipeline: {}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                }
            }
        }
    }

    impl HttpEntry {
        fn read_client(buffer_size: usize, client: &mut TcpEntryContext) -> Result<(), Error> {
            // let mut buffer = vec![0u8; buffer_size];
            // let size = client.connection.read(&mut buffer)?;
            // client.connection_buf.extend(buffer[0..size].to_vec());
            // Ok(())
        }

        fn read_pipeline(client: &mut TcpEntryContext) -> Result<(), Error> {
            client.pipeline_buf.extend(client.pipeline.read_pipeline()?);
            Ok(())
        }

        fn write_client(client: &mut TcpEntryContext) -> Result<(), Error> {
            if client.pipeline_buf.len() > 0 {
                client.connection.write(client.pipeline_buf.as_slice())?;
                client.pipeline_buf.clear();
            }
            Ok(())
        }

        fn write_pipeline(client: &mut TcpEntryContext) -> Result<(), Error> {
            if client.connection_buf.len() > 0 {
                client
                    .pipeline
                    .write_pipeline(client.connection_buf.clone())?;
                client.connection_buf.clear();
            }
            Ok(())
        }
    }

    impl EntryStatic<TcpEntry> for TcpEntry {
        fn new(
            args: CliParsed,
            pipeline: crate::Pipeline,
            debug_level: DebugLevel,
        ) -> Result<TcpEntry, Error> {
            let address = match args.argument_values.get(TCP_ENTRY_ADDRESS.0) {
                Some(address) => address[0].clone(),
                None => return Err(Error::RequireOption(TCP_ENTRY_ADDRESS.0.to_string())),
            };
            let port = match args.argument_values.get(TCP_ENTRY_PORT.0) {
                Some(port) => port[0].clone(),
                None => return Err(Error::RequireOption(TCP_ENTRY_PORT.0.to_string())),
            };
            let buffer_size = match args.argument_values.get(BUFFER_SIZE.0) {
                Some(buffer_size) => buffer_size[0].clone(),
                None => return Err(Error::RequireOption(BUFFER_SIZE.0.to_string())),
            };

            let port = match str::parse::<u16>(port.as_str()) {
                Ok(port) => port,
                Err(e) => return Err(Error::ParseIntError),
            };
            let buffer_size = match str::parse::<usize>(buffer_size.as_str()) {
                Ok(buffer_size) => buffer_size,
                Err(e) => return Err(Error::ParseIntError),
            };

            Ok(TcpEntry {
                address,
                port,
                debug_level,
                pipeline_template: pipeline,
                connections: MultiMap::new(),
                buffer_size,
            })
        }

        fn get_cmd(mut argument: CliSpec) -> CliSpec {
            argument = argument.add_argument(Argument {
                name: TCP_ENTRY_ADDRESS.0.to_string(),
                key: vec![TCP_ENTRY_ADDRESS.1.to_string()],
                argument_occurrence: ArgumentOccurrence::Single,
                value_type: ArgumentValueType::Single,
                default_value: Some("0.0.0.0".to_string()),
                help: Some(ArgumentHelp::Text(TCP_ENTRY_ADDRESS.2.to_string())),
            });
            argument = argument.add_argument(Argument {
                name: TCP_ENTRY_PORT.0.to_string(),
                key: vec![TCP_ENTRY_PORT.1.to_string()],
                argument_occurrence: ArgumentOccurrence::Single,
                value_type: ArgumentValueType::Single,
                default_value: Some("80".to_string()),
                help: Some(ArgumentHelp::Text(TCP_ENTRY_PORT.2.to_string())),
            });
            argument
        }
    }

    pub struct TcpStep {
        address: String,
        port: u16,
        connection: TcpStream,
        debug_level: DebugLevel,
        buffer_size: usize,
    }

    impl TcpStep {}

    impl Step for TcpStep {
        fn process_data_forward(&mut self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            self.connection.write(&data)?;
            Ok(data.clone())
        }

        fn process_data_backward(&mut self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            let mut read_buffer = vec![0u8; self.buffer_size];
            let read_size = self.connection.read(&mut read_buffer)?;
            Ok(read_buffer[0..read_size].to_vec())
        }
    }

    impl BoxedClone for TcpStep {
        fn bclone(&self) -> Box<dyn Step> {
            Box::new(Self {
                address: self.address.clone(),
                port: self.port.clone(),
                connection: unsafe { TcpStream::from_raw_fd(self.connection.as_raw_fd()) },
                debug_level: self.debug_level,
                buffer_size: self.buffer_size,
            })
        }
    }

    impl StepStatic for TcpStep {
        fn new(args: CliParsed, debug_level: DebugLevel) -> Result<Self, Error> {
            let address = match args.argument_values.get(TCP_STEP_ADDRESS.0) {
                Some(address) => address[0].clone(),
                None => return Err(Error::RequireOption(TCP_STEP_ADDRESS.0.to_string())),
            };
            let port = match args.argument_values.get(TCP_STEP_PORT.0) {
                Some(port) => port[0].clone(),
                None => return Err(Error::RequireOption(TCP_STEP_PORT.0.to_string())),
            };
            let buffer_size = match args.argument_values.get(BUFFER_SIZE.0) {
                Some(buffer_size) => buffer_size[0].clone(),
                None => return Err(Error::RequireOption(BUFFER_SIZE.0.to_string())),
            };

            let port = match str::parse::<u16>(port.as_str()) {
                Ok(port) => port,
                Err(e) => return Err(Error::ParseIntError),
            };
            let buffer_size = match str::parse::<usize>(buffer_size.as_str()) {
                Ok(buffer_size) => buffer_size,
                Err(e) => return Err(Error::ParseIntError),
            };

            let addr = create_socket_addr(address.as_str(), port)?;
            let connection = TcpStream::connect(addr)?;

            Ok(Self {
                address: address,
                port: port,
                connection: connection,
                debug_level: debug_level,
                buffer_size: buffer_size,
            })
        }

        fn get_cmd(mut argument: CliSpec) -> CliSpec {
            argument = argument.add_argument(Argument {
                name: TCP_STEP_ADDRESS.0.to_string(),
                key: vec![TCP_STEP_ADDRESS.1.to_string()],
                argument_occurrence: ArgumentOccurrence::Single,
                value_type: ArgumentValueType::Single,
                default_value: Some("127.0.0.1".to_string()),
                help: Some(ArgumentHelp::Text(TCP_STEP_ADDRESS.2.to_string())),
            });
            argument = argument.add_argument(Argument {
                name: TCP_STEP_PORT.0.to_string(),
                key: vec![TCP_STEP_PORT.1.to_string()],
                argument_occurrence: ArgumentOccurrence::Single,
                value_type: ArgumentValueType::Single,
                default_value: Some("80".to_string()),
                help: Some(ArgumentHelp::Text(TCP_STEP_PORT.2.to_string())),
            });
            argument
        }
    }

    impl Clone for TcpStep {
        fn clone(&self) -> Self {
            Self {
                address: self.address.clone(),
                port: self.port.clone(),
                connection: unsafe { TcpStream::from_raw_fd(self.connection.as_raw_fd()) },
                debug_level: self.debug_level,
                buffer_size: self.buffer_size,
            }
        }
    }

    impl AsRawFd for TcpStep {
        fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
            self.connection.as_raw_fd()
        }
    }
}

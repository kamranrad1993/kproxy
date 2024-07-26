pub mod tcp {
    use std::borrow::BorrowMut;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::SocketAddr;
    use std::rc::Rc;
    use std::sync::Arc;
    use std::time::Duration;
    use std::vec;

    use cliparser::types::{
        Argument, ArgumentHelp, ArgumentOccurrence, ArgumentValueType, CliParsed, CliSpec,
    };
    use mio::net::{TcpListener, TcpStream};
    use mio::{Events, Interest, Poll, Token};

    use crate::{
        base::base::DebugLevel, BoxedClone, Entry, EntryStatic, Error, Pipeline, Step, StepStatic,
    };
    use crate::{create_socket_addr, BUFFER_SIZE};

    const TCP_ENTRY_ADDRESS: (&str, &str, &str) = (
        "tcp-entry-address",
        "--tcp-ea",
        "(TcpEntry) Tcp Entry listen address",
    );
    const TCP_ENTRY_PORT: (&str, &str, &str) = (
        "tcp-entry-port",
        "--tcp-ep",
        "(TcpEntry) Tcp Entry listen port",
    );

    const SERVER_TOKEN: Token = Token(0);

    pub struct TcpEntry {
        address: String,
        port: u16,
        debug_level: DebugLevel,
        pipeline_template: Pipeline,
        connections: HashMap<Token, Box<(TcpStream, Pipeline, Vec<u8>)>>,
        buffer_size: usize,
    }

    impl Entry for TcpEntry {
        fn listen(&mut self) -> Result<(), Error> {
            let addr = create_socket_addr(self.address.as_str(), self.port)?;
            let mut server = TcpListener::bind(addr)?;

            let mut poll = Poll::new()?;
            let mut events = Events::with_capacity(128);
            poll.registry()
                .register(&mut server, SERVER_TOKEN, Interest::READABLE)?;
            let mut connection_counter = 0;

            loop {
                poll.poll(&mut events, Some(Duration::from_millis(10)))?;
                for event in events.iter() {
                    match event.token() {
                        SERVER_TOKEN => {
                            let mut connection = server.accept()?;

                            let mut client = Box::new((connection.0, self.pipeline_template.clone(), vec![0u8; 0]));

                            connection_counter += 1;
                            let token = Token(connection_counter);
                            connection_counter += 1;
                            let pipeline_token = Token(connection_counter);

                            poll.registry().register(
                                &mut client.as_mut().0,
                                token,
                                Interest::READABLE | Interest::WRITABLE,
                            )?;
                            poll.registry().register(
                                &mut client.as_mut().1,
                                pipeline_token,
                                Interest::READABLE | Interest::WRITABLE,
                            )?;

                            self.connections.insert(
                                token,
                                client,
                            );
                            // drop(connection);
                        }
                        other => {
                            let client = match self.connections.get_mut(&other) {
                                Some(client) => client,
                                None => {
                                    eprintln!("No client found with this token {}", other.0);
                                    continue;
                                }
                            };

                            if event.is_readable() {
                                TcpEntry::read_client(self.buffer_size, client)?;
                            }

                            // if event.is_writable() {
                            //     TcpEntry::write_client(client)?;
                            // }
                        }
                        _ => unreachable!(),
                    }
                }
            }
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
                connections: HashMap::new(),
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

    impl TcpEntry {
        fn read_client(
            buffer_size: usize,
            client: &mut (TcpStream, Pipeline, Vec<u8>),
        ) -> Result<(), Error> {
            let mut buffer = vec![0u8; buffer_size];
            let size = client.0.read(&mut buffer)?;
            for step in client.1.iter_forwad() {
                buffer = step.process_data_forward(buffer[0..size].to_vec().as_mut())?;
            }
            client.2.extend(buffer);
            Ok(())
        }

        fn write_client(client: &mut (TcpStream, Pipeline, Vec<u8>)) -> Result<(), Error> {
            for step in client.1.iter_backward() {
                client.2 = step.process_data_backward(&mut client.2)?;
            }
            client.0.write(client.2.as_slice())?;
            client.2.clear();
            Ok(())
        }
    }

    pub struct TcpStep {}

    impl Step for TcpStep {
        fn process_data_forward(&self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            todo!()
        }

        fn process_data_backward(&self, data: &mut Vec<u8>) -> Result<Vec<u8>, Error> {
            todo!()
        }
    }

    impl BoxedClone for TcpStep {
        fn bclone(&self) -> Box<dyn Step> {
            todo!()
        }
    }

    impl StepStatic for TcpStep {
        fn new(args: CliParsed, debug_level: DebugLevel) -> Result<Self, Error> {
            todo!()
        }

        fn get_cmd(mut argument: CliSpec) -> CliSpec {
            todo!()
        }
    }

    impl Copy for TcpStep {}

    impl Clone for TcpStep {
        fn clone(&self) -> Self {
            todo!()
        }
    }

    impl AsRawFd for TcpStep{
        fn as_raw_fd(&self) -> std::os::unix::prelude::RawFd {
            todo!()
        }
    }
}

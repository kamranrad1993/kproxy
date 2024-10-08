use std::env;
use std::io::Read;
use std::process::exit;

use kproxy::{
    DebugLevel, Entry, EntryStatic, HttpEntry, Pipeline, StdioEntry, StdioStep, StepStatic,
    TcpEntry, TcpStep, BUFFER_SIZE,
};

use cliparser::types::{
    Argument, ArgumentHelp, ArgumentOccurrence, ArgumentValueType, CliParsed, CliSpec,
    CliSpecMetaInfo,
};
use cliparser::{help, parse, version};

const HELP: (&str, &str, &str, &str) = ("Help", "--help", "-h", "Show help and exit");
const VERSION: (&str, &str, &str, &str) = ("Version", "--version", "-v", "Show version and exit");
const DEBUG_LEVEL: (&str, &str, &str, &str) = (
    "Debug",
    "--debug",
    "-d",
    "Debug Level from 0 to 3 (0 wouldnt show anything).",
);
const ENTRY: (&str, &str, &str, &str) = ("Entry", "--entry", "-e", "Entry step of pipeline");
const STEP: (&str, &str, &str, &str) = ("Step", "--step", "-s", "Step of pipeline");

fn main() {
    let mut cli_spec = CliSpec::new();
    cli_spec = cli_spec.set_meta_info(Some(CliSpecMetaInfo {
        author: Some("Kamran Raei".to_string()),
        version: Some("0.0.1".to_string()),
        description: Some("proxy pipeline".to_string()),
        project: Some("kproxy".to_string()),
        help_post_text: Some(
            "See more info at: https://github.com/kamranrad1993/kproxy".to_string(),
        ),
    }));
    cli_spec = cli_spec.add_argument(Argument {
        name: VERSION.0.to_string(),
        key: vec![VERSION.1.to_string(), VERSION.2.to_string()],
        argument_occurrence: ArgumentOccurrence::Multiple,
        value_type: ArgumentValueType::None,
        default_value: None,
        help: Some(ArgumentHelp::Text(VERSION.3.to_string())),
    });
    cli_spec = cli_spec.add_argument(Argument {
        name: HELP.0.to_string(),
        key: vec![HELP.1.to_string(), HELP.2.to_string()],
        argument_occurrence: ArgumentOccurrence::Multiple,
        value_type: ArgumentValueType::None,
        default_value: None,
        help: Some(ArgumentHelp::Text(HELP.3.to_string())),
    });
    cli_spec = cli_spec.add_argument(Argument {
        name: DEBUG_LEVEL.0.to_string(),
        key: vec![DEBUG_LEVEL.1.to_string(), DEBUG_LEVEL.2.to_string()],
        argument_occurrence: ArgumentOccurrence::Single,
        value_type: ArgumentValueType::Single,
        default_value: Some("0".to_string()),
        help: Some(ArgumentHelp::Text(DEBUG_LEVEL.3.to_string())),
    });

    cli_spec = cli_spec.add_argument(Argument {
        name: ENTRY.0.to_string(),
        key: vec![ENTRY.1.to_string(), ENTRY.2.to_string()],
        argument_occurrence: ArgumentOccurrence::Single,
        value_type: ArgumentValueType::Single,
        default_value: None,
        help: Some(ArgumentHelp::Text(ENTRY.3.to_string())),
    });

    cli_spec = cli_spec.add_argument(Argument {
        name: STEP.0.to_string(),
        key: vec![STEP.1.to_string(), STEP.2.to_string()],
        argument_occurrence: ArgumentOccurrence::Multiple,
        value_type: ArgumentValueType::Single,
        default_value: None,
        help: Some(ArgumentHelp::Text(STEP.3.to_string())),
    });

    cli_spec = cli_spec.add_argument(Argument {
        name: BUFFER_SIZE.0.to_string(),
        key: vec![BUFFER_SIZE.1.to_string(), BUFFER_SIZE.2.to_string()],
        argument_occurrence: ArgumentOccurrence::Single,
        value_type: ArgumentValueType::Single,
        default_value: Some("8192".to_string()),
        help: Some(ArgumentHelp::Text(BUFFER_SIZE.3.to_string())),
    });

    cli_spec = StdioEntry::get_cmd(cli_spec);
    cli_spec = StdioStep::get_cmd(cli_spec);
    cli_spec = TcpEntry::get_cmd(cli_spec);
    cli_spec = TcpStep::get_cmd(cli_spec);
    cli_spec = HttpEntry::get_cmd(cli_spec);

    let args = Vec::from_iter(env::args());
    let args = args
        .iter()
        .skip(1)
        .map(AsRef::as_ref)
        .collect::<Vec<&str>>();

    let mut debug_level = DebugLevel::None;

    let result = parse(&args, &cli_spec);
    match result {
        Ok(cli_parsed) => {
            if cli_parsed.arguments.contains(DEBUG_LEVEL.0) {
                debug_level = match cli_parsed.argument_values.get(DEBUG_LEVEL.0) {
                    Some(debug_str) => {
                        let i = str::parse::<i32>(debug_str[0].as_str()).unwrap();
                        DebugLevel::from(i)
                    }
                    None => {
                        eprintln!("No devug level mentioned");
                        exit(1);
                    }
                }
            }

            if cli_parsed.arguments.contains(VERSION.0) {
                let version_text = version(&cli_spec);
                println!("{}", version_text);
                exit(0);
            } else if cli_parsed.arguments.contains(HELP.0) {
                let help_text = help(&cli_spec);
                println!("{}", help_text);
                exit(0);
            } else {
                run(cli_parsed, debug_level);
            }
        }
        Err(e) => match e {
            cliparser::types::ParserError::InvalidCommandLine(e)
            | cliparser::types::ParserError::InvalidCliSpec(e)
            | cliparser::types::ParserError::CommandDoesNotMatchSpec(e)
            | cliparser::types::ParserError::InternalError(e) => {
                eprintln!("{}", e);
                let help_text = help(&cli_spec);
                println!("{}", help_text);
                exit(1);
            }
        },
    }

    // generate help text

    // generate version text
}

fn run(cli_parsed: CliParsed, debug_level: DebugLevel) {
    let entry = match cli_parsed.argument_values.get(ENTRY.0) {
        Some(entry) => entry[0].as_str(),
        None => {
            eprintln!("No Entry was found");
            exit(1);
        }
    };

    let steps = match cli_parsed.argument_values.get(STEP.0) {
        Some(steps) => steps.iter().map(|x| x.as_str()).collect::<Vec<&str>>(),
        None => {
            eprintln!("No Step Was Found");
            exit(1);
        }
    };

    let mut pipeline = Pipeline::new();
    for step in steps {
        match Some(step) {
            Some("stdio") => pipeline.add_step(Box::new(
                StdioStep::new(cli_parsed.clone(), debug_level).unwrap(),
            )),
            Some("tcp") => pipeline.add_step(Box::new(
                TcpStep::new(cli_parsed.clone(), debug_level).unwrap(),
            )),
            Some(_) => {
                eprintln!("Unknown step");
                exit(1);
            }
            None => todo!(),
        }
    }

    match Some(entry) {
        Some("stdio") => {
            let mut entry = StdioEntry::new(cli_parsed.clone(), pipeline, debug_level).unwrap();
            entry.listen().unwrap();
        }
        Some("tcp") => {
            let mut entry = TcpEntry::new(cli_parsed.clone(), pipeline, debug_level).unwrap();
            entry.listen().unwrap();
        }
        Some("http") => {
            let mut entry = HttpEntry::new(cli_parsed.clone(), pipeline, debug_level).unwrap();
            entry.listen().unwrap();
        }
        Some(_) => {
            eprintln!("Unknown entry");
            exit(1);
        }
        None => todo!(),
    };
}




// use std::error::Error;

// use h2::server::{self, SendResponse};
// use h2::RecvStream;
// use hyper::body::Bytes;
// use hyper::{Request, Response};
// use tokio::net::{TcpListener, TcpStream};
// // use tokio::io::

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
//     // let _ = env_logger::try_init();

//     let listener = TcpListener::bind("127.0.0.1:6666").await?;

//     println!("listening on {:?}", listener.local_addr());

//     loop {
//         if let Ok((socket, _peer_addr)) = listener.accept().await {
//             tokio::spawn(async move {
//                 if let Err(e) = serve(socket).await {
//                     println!("  -> err={:?}", e);
//                 }
//             });
//         }
//     }
// }

// async fn serve(socket: TcpStream) -> Result<(), Box<dyn Error + Send + Sync>> {
//     let mut connection = server::handshake(socket).await?;
//     println!("H2 connection bound");

//     while let Some(result) = connection.accept().await {
//         let (request, respond) = result?;
//         tokio::spawn(async move {
//             if let Err(e) = handle_request(request, respond).await {
//                 println!("error while handling request: {}", e);
//             }
//         });
//     }

//     println!("~~~~~~~~~~~ H2 connection CLOSE !!!!!! ~~~~~~~~~~~");
//     Ok(())
// }

// async fn handle_request(
//     mut request: Request<RecvStream>,
//     mut respond: SendResponse<Bytes>,
// ) -> Result<(), Box<dyn Error + Send + Sync>> {
//     println!("GOT request: {:?}", request);

//     let body = request.body_mut();
//     while let Some(data) = body.data().await {
//         let data = data?;
//         println!("<<<< recv {:?}", data);
//         let _ = body.flow_control().release_capacity(data.len());
//     }

//     let response = Response::new(());
//     let mut send = respond.send_response(response, false)?;
//     println!(">>>> send");
//     send.send_data(Bytes::from_static(b"hello "), false)?;
//     send.send_data(Bytes::from_static(b"world\n"), true)?;

//     Ok(())
// }
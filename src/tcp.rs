pub mod tcp {

    use std::collections::HashMap;

    use cliparser::types::{CliParsed, CliSpec};

    use crate::{base::base::DebugLevel, BoxedClone, Entry, EntryStatic, Error, Step, StepStatic};
    struct TcpEntry {}

    impl Entry for TcpEntry {
        fn listen(&mut self) -> Result<(), crate::Error> {
            todo!()
        }
    }

    impl EntryStatic for TcpEntry {
        fn new(
            args: CliParsed,
            pipeline: crate::Pipeline,
            debug_level: DebugLevel,
        ) -> Self {
            todo!()
        }

        fn get_cmd(argument: CliSpec) -> CliSpec {
            todo!()
        }
    }

    struct TcpStep {}

    impl Step for TcpStep {
        fn process_data_forward(&self, data: Vec<u8>) -> Result<Vec<u8>, Error> {
            todo!()
        }

        fn process_data_backward(&self, data: Vec<u8>) -> Result<Vec<u8>, Error> {
            todo!()
        }
    }

    impl BoxedClone for TcpStep {
        fn bclone(&self) -> Box<dyn Step> {
            todo!()
        }
    }

    impl StepStatic for TcpStep {
        fn new(
            args: CliParsed,
            debug_level: DebugLevel,
        ) -> Self {
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
}

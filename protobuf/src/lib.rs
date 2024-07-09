mod generated;

use crate::enums_clientserver::EMsg;
use crate::steammessages_clientserver_appinfo::CMsgClientPICSProductInfoResponse;
pub use generated::*;
use std::fmt::Debug;
use std::io::{Read, Write};

pub trait RpcService {
    const SERVICE_NAME: &'static str;
}

pub trait RpcMethod: protobuf::Message + RpcMessage {
    const METHOD_NAME: &'static str;
    type Response: RpcMessage;
}

pub trait RpcMessage: Debug + Sized {
    fn parse(_reader: &mut dyn Read) -> protobuf::Result<Self>;

    fn write(&self, _writer: &mut dyn Write) -> protobuf::Result<()>;

    fn encode_size(&self) -> usize;
}

impl RpcMessage for () {
    fn parse(_reader: &mut dyn Read) -> protobuf::Result<Self> {
        Ok(())
    }

    fn write(&self, _writer: &mut dyn Write) -> protobuf::Result<()> {
        Ok(())
    }

    fn encode_size(&self) -> usize {
        0
    }
}

pub trait RpcMessageWithKind: RpcMessage {
    const KIND: EMsg;
}

pub trait JobMultiple {
    fn completed(&self) -> bool;
}

impl JobMultiple for CMsgClientPICSProductInfoResponse {
    fn completed(&self) -> bool {
        self.response_pending.unwrap_or(true)
    }
}

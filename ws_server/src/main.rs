#[macro_use] extern crate log;
#[macro_use] extern crate lazy_static;

use ws::{
  listen,
  Message,
  Handshake,
  Error,
  ErrorKind,
  Handler
};

use env_logger::{Builder, Env};
use serde::{Deserialize, Serialize};

use std::sync::{Arc, Mutex};
use std::net::{SocketAddr, Ipv4Addr, IpAddr};

lazy_static! {
  static ref OPERATION_LIST: Mutex<Vec<Operation>> = {
    Mutex::new(Vec::new())
  };
}

fn default_sock_addr() -> SocketAddr { SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 80) }

#[derive(Deserialize, Serialize, Debug)]
struct Operation {
  #[serde(default="default_sock_addr")]
  sender_ip : SocketAddr,
  send_type: String,
  #[serde(default)]
  from_path: Option<String>,
  to_path: String
}

#[derive(Clone, Debug)]
struct Sender {
  ws: ws::Sender,
  addr: SocketAddr,
  clients: Arc<Mutex<Vec<Arc<&'static Sender>>>>,
}

impl Handler for Sender {
  fn on_open(&mut self, hs: Handshake) -> ws::Result<()> {
    if let Some(addr) = hs.peer_addr {
      self.addr = addr;

      let s: &Sender = &*self;
      let s: &'static Sender = unsafe { std::mem::transmute(s) };
      self.clients.lock().unwrap().push(Arc::new(s));
      Ok(())
    } else {
      Err(Error::new(ErrorKind::Internal, "Cannot get IP."))
    }
  }

  fn on_message(&mut self, msg: Message) -> ws::Result<()> {
    let msg_str = &msg.as_text()?;

    if let Ok(mut de) = serde_json::from_str::<Operation>(msg_str) {
      de.sender_ip = self.addr;

      info!("Deserialized = {:?}.", de);
      OPERATION_LIST.lock().unwrap().push(de);

      for client in self.clients.lock().unwrap().iter() {
        for op in OPERATION_LIST.lock().unwrap().iter().filter(|o| o.sender_ip != client.addr) {
          client.ws.send(serde_json::to_string(&op).unwrap())?;
        }
      }
    } else {
      error!("Could not deserialize: \"{}\".", msg_str)
    }

    Ok(())
  }
}

impl Drop for Sender {
  fn drop(&mut self) {
    self.clients.lock().unwrap().retain(|s| s.addr != self.addr)
  }
}

fn main() {
  let env = Env::default()
    .filter_or(env_logger::DEFAULT_FILTER_ENV, "info");

  Builder::from_env(env).init();

  let clients = Arc::new(Mutex::new(Vec::new()));

  if let Err(error) = listen("127.0.0.1:3012", |out| {
    Sender {
      ws: out,
      addr: default_sock_addr(),
      clients: clients.clone(),
    }
  }) {
    error!("Failed to create WebSocket due to {:?}", error);
  }
}

use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::{
    channel, sync_channel, Receiver, RecvError, SendError, Sender, SyncSender, TryRecvError,
};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self, JoinHandle};

use gflux::sync::Obs;
use log::error;
use ssh2::Session;

type ReqId = u64;

pub struct Backend {
    next_req_id: ReqId,
    req_sender: SyncSender<(ReqId, BackendReq)>,
    resp_receiver: Receiver<(ReqId, BackendResp)>,
    callbacks: HashMap<ReqId, Box<dyn Fn(BackendResp)>>,
    wakeup: Arc<dyn Fn()>,
    worker: JoinHandle<()>,
}

impl Backend {
    pub fn ssh(
        user: &str,
        host: &str,
        port: Option<u16>,
        wakeup: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (req_sender, req_receiver) = sync_channel(100);
        let (resp_sender, resp_receiver) = sync_channel(100);
        let config = BackendConfig::Ssh {
            user: user.to_string(),
            host: host.to_string(),
            port,
        };
        let jh = spawn(config, wakeup.clone(), req_receiver, resp_sender);

        Self {
            next_req_id: 1,
            req_sender,
            resp_receiver,
            callbacks: HashMap::new(),
            wakeup,
            worker: jh,
        }
    }

    pub fn handle_responses(&mut self) {
        loop {
            match self.resp_receiver.try_recv() {
                Ok((req_id, resp)) => {
                    if let Some(cb) = self.callbacks.remove(&req_id) {
                        cb(resp);
                    }
                }
                Err(TryRecvError::Disconnected) => {
                    error!("backend disconnected");
                    break;
                }
                Err(TryRecvError::Empty) => {
                    break;
                }
            }
        }
    }

    pub fn path_exists(&mut self, path: &Path, cb: Box<dyn Fn(bool)>) {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        dbg!(self
            .req_sender
            .send((req_id, BackendReq::Exists(path.to_owned()))));
        self.callbacks.insert(
            req_id,
            Box::new(move |resp| {
                if let BackendResp::Exists(exists) = resp {
                    cb(exists);
                }
            }),
        );
    }

    pub fn list_files(&mut self, path: &Path, cb: Box<dyn Fn(Vec<DirEntry>)>) {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        dbg!(self
            .req_sender
            .send((req_id, BackendReq::List(path.to_owned()))));
        self.callbacks.insert(
            req_id,
            Box::new(move |resp| {
                if let BackendResp::List(entries) = resp {
                    cb(entries);
                }
            }),
        );
    }

    // fn send(&self, e: BackendEvent) -> Result<(), SendError<BackendEvent>> {
    //     let res = self.sender.send(e);
    //     (self.waker)();
    //     res
    // }
}

#[derive(Debug)]
pub enum BackendReq {
    Exists(PathBuf),
    List(PathBuf),
}

#[derive(Debug)]
pub enum BackendResp {
    Exists(bool),
    List(Vec<DirEntry>),
}

#[derive(Debug)]
pub struct ListResp {}

#[derive(Debug)]
pub struct DirEntry {
    file_name: OsString,
}

pub enum BackendWorker {
    Ssh {
        user: String,
        host: String,
        port: Option<u16>,
        sess: Option<Session>,
    },
    Local,
}

impl fmt::Debug for BackendWorker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ssh {
                user,
                host,
                port,
                sess,
            } => f
                .debug_struct("BackendWorker::Ssh")
                .field("user", &user)
                .field("host", &host)
                .field("port", &port)
                .finish(),
            Self::Local => f.debug_struct("BackendWorker::Local").finish(),
        }
    }
}

#[derive(Debug, Default)]
pub enum BackendConfig {
    Ssh {
        user: String,
        host: String,
        port: Option<u16>,
    },
    #[default]
    Local,
}

#[derive(Debug)]
pub enum BackendMsg {
    ListFiles(PathBuf),
}

pub fn spawn(
    config: BackendConfig,
    wakeup: Arc<dyn Fn() + Send + Sync>,
    receiver: Receiver<(ReqId, BackendReq)>,
    sender: SyncSender<(ReqId, BackendResp)>,
) -> JoinHandle<()> {
    thread::spawn(|| {
        main_loop(config, wakeup, receiver, sender);
    })
}

pub fn main_loop(
    config: BackendConfig,
    wakeup: Arc<dyn Fn() + Send + Sync>,
    req_receiver: Receiver<(ReqId, BackendReq)>,
    resp_sender: SyncSender<(ReqId, BackendResp)>,
) -> Result<(), Box<dyn Error>> {
    dbg!("backend main loop");
    if let BackendConfig::Ssh { user, host, port } = config {
        // Connect to the local SSH server
        let tcp = dbg!(TcpStream::connect(host)?);
        let mut sess = Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;
        sess.userauth_agent(&user)?;
        assert!(sess.authenticated());

        // execute a command
        let mut chan = sess.channel_session()?;
        chan.exec("ls")?;
        let mut s = String::new();
        chan.read_to_string(&mut s)?;
        chan.wait_close()?;
        println!("{}", s);

        // list files with sftp
        // let sftp = sess.sftp()?;
        // let mut dir = sftp.opendir(Path::new("/"))?;
        // while let Ok((file, stat)) = dir.readdir() {
        //     println!("{} {:?}", file.display(), stat);
        // }
        // dbg!(dir.readdir());

        loop {
            match req_receiver.recv() {
                Ok((req_id, req)) => {
                    dbg!(&req);
                    match req {
                        BackendReq::Exists(p) => {
                            dbg!(&p);
                        }
                        BackendReq::List(p) => {
                            let sftp = sess.sftp()?;
                            let mut dir = sftp.opendir(&p)?;
                            let mut entries = vec![];
                            while let Ok((file, stat)) = dir.readdir() {
                                if let Some(file_name) = file.file_name() {
                                    entries.push(DirEntry {
                                        file_name: file_name.to_os_string(),
                                    });
                                }
                                println!("{} {:?}", file.display(), stat);
                            }

                            resp_sender.send((req_id, BackendResp::List(entries)));

                            dbg!(&p);
                        }
                    }
                }
                Err(e) => error!("backend failed"),
            }
            wakeup();
        }
    }

    Ok(())
}

#[test]
fn test_auth() {
    // let backend = BackendWorker::ssh("localhost:22".to_string(), Some(22), "brain".to_string());
    // let (sender, receiver) = sync_channel(10);
    // backend.spawn(Box::new(|| println!("wakup")), sender);
}

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::io::Read;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::mpsc::{
    channel, sync_channel, Receiver, RecvError, SendError, Sender, SyncSender, TryRecvError,
};
use std::sync::{mpsc, Arc, RwLock};
use std::thread::{self, sleep, JoinHandle};
use std::time::Duration;

use gflux::sync::Obs;
use log::error;
use ssh2::Session;

use crate::Window;

type ReqId = u64;

pub struct Backend {
    next_req_id: ReqId,
    req_sender: SyncSender<(ReqId, BackendReq)>,
    resp_receiver: PeekableReceiver<(ReqId, BackendResp)>,
    callbacks: HashMap<ReqId, Box<dyn Fn(&mut Window, BackendResp)>>,
    wakeup: Arc<dyn Fn()>,
    worker: JoinHandle<()>,
}

pub struct PeekableReceiver<T> {
    receiver: Receiver<T>,
    cached_resp: Rc<RefCell<Option<Result<T, TryRecvError>>>>,
}

impl<T> PeekableReceiver<T> {
    fn new(receiver: Receiver<T>) -> Self {
        Self {
            receiver,
            cached_resp: Rc::new(RefCell::new(None)),
        }
    }

    fn has_read(&self) -> bool {
        if self.cached_resp.borrow_mut().is_some() {
            return true;
        }

        match self.receiver.try_recv() {
            Ok(t) => {
                *self.cached_resp.borrow_mut() = Some(Ok(t));
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(e) => {
                *self.cached_resp.borrow_mut() = Some(Err(e));
                true
            }
        }
    }

    fn try_recv(&self) -> Result<T, TryRecvError> {
        if let Some(r) = self.cached_resp.borrow_mut().take() {
            return r;
        }

        self.receiver.try_recv()
    }
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
            resp_receiver: PeekableReceiver::new(resp_receiver),
            callbacks: HashMap::new(),
            wakeup,
            worker: jh,
        }
    }

    pub fn has_resp(&self) -> bool {
        dbg!(self.resp_receiver.has_read())
    }

    pub fn try_recv_response_cb(
        &mut self,
    ) -> Option<(BackendResp, Box<dyn Fn(&mut Window, BackendResp)>)> {
        match self.resp_receiver.try_recv() {
            Ok((req_id, resp)) => {
                if let Some(cb) = self.callbacks.remove(&req_id) {
                    Some((resp, cb))
                } else {
                    None
                }
            }
            Err(TryRecvError::Disconnected) => {
                error!("backend disconnected");
                None
            }
            Err(TryRecvError::Empty) => None,
        }
    }

    pub fn path_exists(&mut self, path: &Path, cb: Box<dyn Fn(&mut Window, bool)>) {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        dbg!(self
            .req_sender
            .send((req_id, BackendReq::Exists(path.to_owned()))));
        self.callbacks.insert(
            req_id,
            Box::new(move |win, resp| {
                if let BackendResp::Exists(exists) = resp {
                    cb(win, exists);
                }
            }),
        );
    }

    pub fn list_files(&mut self, path: &Path, cb: Box<dyn Fn(&mut Window, Vec<DirEntry>)>) {
        let req_id = self.next_req_id;
        self.next_req_id += 1;
        dbg!(self
            .req_sender
            .send((req_id, BackendReq::List(path.to_owned()))));
        self.callbacks.insert(
            req_id,
            Box::new(move |win, resp| {
                if let BackendResp::List(entries) = resp {
                    cb(win, entries);
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

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: OsString,
    pub mode: u32,
}

impl DirEntry {
    pub fn is_dir(&self) -> bool {
        dbg!(dbg!(self.mode) & 0o0170000 == 0o0040000)
    }
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
        loop {
            if let Err(e) =
                ssh_do_main_loop(&user, &host, port, &wakeup, &req_receiver, &resp_sender)
            {
                error!("ssh: {}", e);
                sleep(Duration::from_secs(1));
            }
        }
    }

    Ok(())
}

pub fn ssh_do_main_loop(
    user: &str,
    host: &str,
    port: Option<u16>,
    wakeup: &Arc<dyn Fn() + Send + Sync>,
    req_receiver: &Receiver<(ReqId, BackendReq)>,
    resp_sender: &SyncSender<(ReqId, BackendResp)>,
) -> Result<(), Box<dyn Error>> {
    dbg!("backend main loop");

    // Connect to the local SSH server
    let tcp = dbg!(TcpStream::connect(host.clone())?);
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
        if let Err(e) = handle_backend_req(&wakeup, &req_receiver, &resp_sender, &sess) {
            error!("backend req error: {}", e);
        }
    }

    Ok(())
}

pub fn handle_backend_req(
    wakeup: &Arc<dyn Fn() + Send + Sync>,
    req_receiver: &Receiver<(ReqId, BackendReq)>,
    resp_sender: &SyncSender<(ReqId, BackendResp)>,
    sess: &ssh2::Session,
) -> Result<(), Box<dyn Error>> {
    match req_receiver.recv() {
        Ok((req_id, req)) => {
            dbg!(&req);
            match req {
                BackendReq::Exists(p) => {
                    dbg!(&p);
                }
                BackendReq::List(p) => {
                    let sftp = sess.sftp()?;
                    dbg!(&p);
                    let mut dir = sftp.opendir(&p)?;
                    let mut entries = vec![];
                    while let Ok((file, stat)) = dir.readdir() {
                        if let Some(file_name) = file.file_name() {
                            entries.push(DirEntry {
                                name: file_name.to_os_string(),
                                mode: stat.perm.unwrap_or_default(),
                            });
                        }
                        println!("{} {:?}", file.display(), stat);
                    }

                    resp_sender.send((req_id, BackendResp::List(entries)))?;

                    dbg!(&p);
                }
            }
        }
        Err(e) => error!("backend failed {}", e),
    }
    wakeup();
    Ok(())
}

#[test]
fn test_auth() {
    // let backend = BackendWorker::ssh("localhost:22".to_string(), Some(22), "brain".to_string());
    // let (sender, receiver) = sync_channel(10);
    // backend.spawn(Box::new(|| println!("wakup")), sender);
}

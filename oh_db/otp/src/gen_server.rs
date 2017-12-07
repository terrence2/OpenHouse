use std::thread;
use std::thread::JoinHandle;
use std::marker::Send;
use std::sync::{Arc, Barrier, Condvar, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};

pub mod errors {
    error_chain!{}
}
use gen_server::errors::{Result, ResultExt};

pub struct GenServer<CastRequest, CallRequest, Response>
where
    CastRequest: Send + 'static,
    CallRequest: Send + 'static,
    Response: Send + 'static,
{
    tx: Sender<__Envelope__<CastRequest, CallRequest>>,
    rx: Mutex<Receiver<Response>>,
    handle: JoinHandle<Result<()>>,
}

impl<CastRequest: Send + 'static, CallRequest: Send + 'static, Response: Send + 'static>
    GenServer<CastRequest, CallRequest, Response> {
    pub fn cast(&self, request: CastRequest) -> Result<()> {
        self.tx.send(__Envelope__::Cast(request)).chain_err(
            || "failed to cast",
        )?;
        return Ok(());
    }

    pub fn call(&self, request: CallRequest) -> Result<Response> {
        // Note: we lock the return channel before sending so that concurrent calls do not
        //       interleave after send and before wait
        let return_channel = match self.rx.lock() {
            Ok(chan) => chan,
            Err(e) => bail!("failed to lock return chan: {}", format!("{:?}", e)),
        };
        self.tx.send(__Envelope__::Call(request)).chain_err(
            || "failed to send call message",
        )?;
        let result = return_channel.recv().chain_err(
            || "failed to recv call response",
        )?;
        return Ok(result);
    }

    pub fn stop(self) -> Result<()> {
        self.tx.send(__Envelope__::Terminate).chain_err(
            || "failed send stop",
        )?;
        return match self.handle.join() {
            Ok(result) => Ok(result.chain_err(|| "")?),
            Err(error) => bail!(format!("failed to join: {:?}", error)),
        };
    }
}

pub enum __Envelope__<CastRequest, CallRequest>
where
    CastRequest: Send + 'static,
    CallRequest: Send + 'static,
{
    Terminate,
    Cast(CastRequest),
    Call(CallRequest),
}

pub trait GenServerWorker {
    type CastRequest: Send + 'static;
    type CallRequest: Send + 'static;
    type Response: Send + 'static;

    // The state should default to Self, but is not yet supported (see issue #29661).
    type State;

    fn start_link() -> Result<GenServer<Self::CastRequest, Self::CallRequest, Self::Response>> {
        let (tx, rx) = channel();
        let (tx_return, rx_return) = channel();

        // Start child and wait until init is done before returning.
        let init_done_parent = Arc::new(Barrier::new(2));
        let init_done_child = init_done_parent.clone();
        let handle = thread::spawn(move || {
            Self::__server_loop__(rx, tx_return, init_done_child)
        });
        init_done_parent.wait();

        return Ok(GenServer {
            tx,
            rx: Mutex::new(rx_return),
            handle,
        });
    }

    fn __server_loop__(
        rx: Receiver<__Envelope__<Self::CastRequest, Self::CallRequest>>,
        tx_return: Sender<Self::Response>,
        init_done: Arc<Barrier>,
    ) -> Result<()> {
        let mut state = Self::init().chain_err(|| "tree server init")?;
        init_done.wait();
        loop {
            let envelope = rx.recv().chain_err(|| "tree server recv")?;
            state = match envelope {
                __Envelope__::Terminate => {
                    Self::terminate(state);
                    return Ok(());
                }
                __Envelope__::Cast(req) => {
                    Self::handle_cast(req, state).chain_err(|| "handle error")?
                }
                __Envelope__::Call(req) => {
                    let (result, state) =
                        Self::handle_call(req, state).chain_err(|| "handle error")?;
                    tx_return.send(result).chain_err(|| "send response")?;
                    state
                }
            }
        }
    }

    fn init() -> Result<Self::State>;
    fn terminate(_state: Self::State) {}
    fn handle_cast(msg: Self::CastRequest, state: Self::State) -> Result<Self::State>;
    fn handle_call(
        msg: Self::CallRequest,
        state: Self::State,
    ) -> Result<(Self::Response, Self::State)>;
}

#[cfg(test)]
mod tests {
    use gen_server::*;

    struct MathServer {
        v: i32,
    }
    enum Message {
        Add(i32),
        Sub(i32),
        Mul(i32),
        Div(i32),
    }
    enum Request {
        Get,
    }
    impl GenServerWorker for MathServer {
        type State = Self;
        type CastRequest = Message;
        type CallRequest = Request;
        type Response = i32;

        fn init() -> Result<Self> {
            return Ok(MathServer { v: 0 });
        }

        fn terminate(state: Self) {
            if state.v != 42 {
                panic!("incorrect value at terminate");
            }
        }

        fn handle_cast(request: Message, mut state: Self) -> Result<Self> {
            state.v = match request {
                Message::Add(v) => state.v + v,
                Message::Sub(v) => state.v - v,
                Message::Mul(v) => state.v * v,
                Message::Div(v) => state.v / v,
            };
            return Ok(state);
        }

        fn handle_call(request: Request, state: Self) -> Result<(i32, Self)> {
            return Ok((state.v, state));
        }
    }

    #[test]
    fn it_works() {
        let it = MathServer::start_link().unwrap();
        it.cast(Message::Add(42)).unwrap();
        it.cast(Message::Div(3)).unwrap();
        it.cast(Message::Sub(4)).unwrap();
        it.cast(Message::Mul(4)).unwrap();
        it.cast(Message::Add(2)).unwrap();
        assert_eq!(42, it.call(Request::Get).unwrap());
        it.stop().unwrap();
    }
}

use std::thread;
use std::thread::JoinHandle;
use std::marker::Send;
use std::sync::{Arc, Barrier};
use std::sync::mpsc::{channel, Receiver, Sender};

pub mod errors {
    error_chain!{}
}
use ::gen_server::errors::{Result, ResultExt};

pub struct GenServer<Message: Send + 'static> {
    tx: Sender<__Envelope__<Message>>,
    handle: JoinHandle<Result<()>>,
}

impl<Message: Send + 'static> GenServer<Message> {
    pub fn cast(&self, msg: Message) -> Result<()> {
        self.tx.send(__Envelope__::Cast(msg)).chain_err(|| "failed to cast")?;
        return Ok(());
    }

    pub fn stop(self) -> Result<()> {
        self.tx.send(__Envelope__::Terminate).chain_err(|| "failed send stop")?;
        return match self.handle.join() {
            Ok(result) => Ok(result.chain_err(|| "")?),
            Err(error) => bail!(format!("failed to join: {:?}", error))
        };
    }
}

pub enum __Envelope__<Message: Send + 'static> {
    Terminate,
    Cast(Message)
}

pub trait GenServerWorker
{
    type Message : Send;

    // The state should default to Self, but is not yet supported (see issue #29661).
    type State;

    fn start_link() -> Result<GenServer<Self::Message>> {
        let (tx, rx) = channel();
        let init_done_parent = Arc::new(Barrier::new(2));
        let init_done_child = init_done_parent.clone();
        let handle = thread::spawn(move || {
            Self::__server_loop__(rx, init_done_child)
        });
        init_done_parent.wait();
        return Ok(GenServer {tx, handle});
    }

    fn __server_loop__(rx: Receiver<__Envelope__<Self::Message>>, init_done: Arc<Barrier>) -> Result<()> {
        let mut state = Self::init().chain_err(|| "tree server init")?;
        init_done.wait();
        loop {
            let envelope = rx.recv().chain_err(|| "tree server recv")?;
            state = match envelope {
                __Envelope__::Terminate => { Self::terminate(state); return Ok(()) },
                __Envelope__::Cast(msg) => Self::handle_cast(msg, state).chain_err(|| "handle error")?
            }
        }
    }

    fn init() -> Result<Self::State>;
    fn terminate(_state: Self::State) {}
    fn handle_cast(msg: Self::Message, state: Self::State) -> Result<Self::State>;
}

#[cfg(test)]
mod tests {
    use ::gen_server::*;

    struct MathServer {
        v: i32
    }
    enum Message {
        Add(i32),
        Sub(i32),
        Mul(i32),
        Div(i32),
    }
    impl GenServerWorker for MathServer {
        type State = Self;
        type Message = Message;

        fn init() -> Result<Self> {
            return Ok(MathServer { v: 0});
        }

        fn terminate(state: Self) {
            if state.v != 42 {
                panic!("incorrect value at terminate");
            }
        }

        fn handle_cast(msg: Self::Message, mut state: Self) -> Result<Self> {
            state.v = match msg {
                Message::Add(v) => state.v + v,
                Message::Sub(v) => state.v - v,
                Message::Mul(v) => state.v * v,
                Message::Div(v) => state.v / v
            };
            return Ok(state);
        }
    }

    #[test]
    fn it_works() {
        let it: GenServer<Message> = MathServer::start_link().unwrap();
        it.cast(Message::Add(42)).unwrap();
        it.cast(Message::Div(3)).unwrap();
        it.cast(Message::Sub(4)).unwrap();
        it.cast(Message::Mul(4)).unwrap();
        it.cast(Message::Add(2)).unwrap();
        //assert_eq!(42, it.call(Message::Get).unwrap());
        it.stop().unwrap();
    }
}

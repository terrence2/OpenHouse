use otp::gen_server::{GenServer, GenServerWorker};
use otp::gen_server::errors::{Result as GenServerResult, ResultExt};
use std::collections::HashMap;
use yggdrasil::{Tree, TreeChanges};
use yggdrasil::{Glob, Path};

pub mod errors {
    error_chain!{
        links {
            GenServerError(::otp::gen_server::errors::Error, ::otp::gen_server::errors::ErrorKind);
        }
    }
}
use ::tree_server::errors::{Result};

pub enum CastRequest {
    CreateDirectory(Path, String),
    CreateFile(Path, String),
    CreateFormula(Path, String, HashMap<String, Path>, String),
    RemoveNode(Path, String),
}

pub enum CallRequest {
    ListDirectory(Path),
    GetFile(Path),
    GetMatchingFiles(Glob),
    SetFile(Path, String),
    SetMatchingFiles(Glob, String),
}

pub enum Response {
    DirectoryList(Vec<String>),
    FileData(String),
    MatchingFileData(Vec<(Path, String)>),
    Changes(TreeChanges),
}

pub struct Worker {
    tree: Tree
}

impl GenServerWorker for Worker {
    type State = Self;
    type CastRequest = CastRequest;
    type CallRequest = CallRequest;
    type Response = Response;

    fn init() -> GenServerResult<Self> {
        return Ok(Worker { tree: Tree::new() });
    }

    fn handle_cast(request: CastRequest, mut state: Self) -> GenServerResult<Self> {
        match request {
            CastRequest::CreateDirectory(parent_path, name) => {
                state.tree
                    .lookup_directory(&parent_path).chain_err(|| "lookup directory")?
                    .add_directory(&name).chain_err(|| "add directory")?;
            }
            CastRequest::CreateFile(parent_path, name) => {
                state.tree
                    .lookup_directory(&parent_path).chain_err(|| "lookup directory")?
                    .add_file(&name).chain_err(|| "add file")?;
            }
            CastRequest::CreateFormula(parent_path, name, inputs, formula) => {
                state.tree
                    .create_formula(&parent_path, &name, &inputs, &formula)
                    .chain_err(|| "create formula")?;
            }
            CastRequest::RemoveNode(parent_path, name) => {
                state.tree
                    .lookup_directory(&parent_path).chain_err(|| "lookup directory")?
                    .remove_child(&name).chain_err(|| "remove node")?;
            }
        };
        return Ok(state);
    }

    fn handle_call(request: CallRequest, mut state: Self) -> GenServerResult<(Response, Self)> {
        let response = match request {
            CallRequest::ListDirectory(path) =>
                Response::DirectoryList(state.tree
                    .lookup_directory(&path).chain_err(|| "lookup directory")?
                    .list_directory()),
            CallRequest::GetFile(path) =>
                Response::FileData(state.tree.get_data_at(&path).chain_err(|| "get_data_at")?),
            CallRequest::GetMatchingFiles(glob) =>
                Response::MatchingFileData(state.tree.get_data_matching(&glob).chain_err(|| "get_data_matching")?),
            CallRequest::SetFile(path, content) =>
                Response::Changes(state.tree.set_data_at(&path, &content).chain_err(|| "set_file_data")?),
            CallRequest::SetMatchingFiles(glob, content) =>
                Response::Changes(state.tree.set_data_matching(&glob, &content).chain_err(|| "set_data_matching")?),
        };
        return Ok((response, state));
    }
}

pub struct TreeServer {
    gen_server: GenServer<CastRequest, CallRequest, Response>
}

impl TreeServer {
    pub fn start_link() -> Result<TreeServer> {
        Ok(TreeServer {gen_server: Worker::start_link().chain_err(|| "start_link wrapper")?})
    }

    pub fn create_directory(&self, parent_path: Path, name: &str) -> Result<()> {
        Ok(self.gen_server.cast(CastRequest::CreateDirectory(parent_path, name.to_owned())).chain_err(|| "cast")?)
    }

    pub fn create_file(&self, parent_path: Path, name: &str) -> Result<()> {
        Ok(self.gen_server.cast(CastRequest::CreateFile(parent_path, name.to_owned())).chain_err(|| "cast")?)
    }

    pub fn create_formula(&self, parent_path: Path, name: &str, inputs: HashMap<String, Path>, formula: &str) -> Result<()> {
        Ok(self.gen_server.cast(CastRequest::CreateFormula(parent_path, name.to_owned(), inputs, formula.to_owned())).chain_err(|| "cast")?)
    }

    pub fn remove_node(&self, parent_path: Path, name: &str) -> Result<()> {
        Ok(self.gen_server.cast(CastRequest::RemoveNode(parent_path, name.to_owned())).chain_err(|| "cast")?)
    }

    pub fn list_directory(&self, path: Path) -> Result<Vec<String>> {
        let response = self.gen_server.call(CallRequest::ListDirectory(path)).chain_err(|| "call")?;
        if let Response::DirectoryList(children) = response { return Ok(children); }
        bail!("wrong response type for list_directory");
    }

    pub fn get_file(&self, path: Path) -> Result<String> {
        let response = self.gen_server.call(CallRequest::GetFile(path)).chain_err(|| "call")?;
        if let Response::FileData(data) = response { return Ok(data); }
        bail!("wrong response type for get_data");
    }

    pub fn get_matching_files(&self, glob: Glob) -> Result<Vec<(Path, String)>> {
        let response = self.gen_server.call(CallRequest::GetMatchingFiles(glob)).chain_err(|| "call")?;
        if let Response::MatchingFileData(data) = response { return Ok(data); }
        bail!("wrong response type for get_data");
    }

    pub fn set_file(&self, path: Path, data: &str) -> Result<TreeChanges> {
        let response = self.gen_server.call(CallRequest::SetFile(path, data.to_owned())).chain_err(|| "call")?;
        if let Response::Changes(changes) = response { return Ok(changes); }
        bail!("wrong response type for set_data");
    }

    pub fn set_matching_files(&self, glob: Glob, data: &str) -> Result<TreeChanges> {
        let response = self.gen_server.call(CallRequest::SetMatchingFiles(glob, data.to_owned())).chain_err(|| "call")?;
        if let Response::Changes(changes) = response { return Ok(changes); }
        bail!("wrong response type for set_matching_data");
    }
}

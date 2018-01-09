use otp::gen_server::{GenServer, GenServerWorker};
use otp::gen_server::errors::{Result as GenServerResult, ResultExt};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use yaml_rust::{Yaml, YamlLoader};
use yggdrasil::{DirectoryData, FormulaData, Tree, TreeChanges};
use yggdrasil::{Glob, Path, PathBuilder};

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
    SeedDatabase(String),
}

pub enum Response {
    DirectoryList(Vec<String>),
    FileData(String),
    MatchingFileData(Vec<(Path, String)>),
    Changes(TreeChanges),
    SeedStatus(Result<()>)
}

pub struct Worker {
    tree: Tree
}

impl Worker {
    fn seed_tree(&mut self, path: String) -> Result<()> {
        let mut file = File::open(path).chain_err(|| "open file")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).chain_err(|| "read file")?;
        let docs = YamlLoader::load_from_str(&contents).chain_err(|| "parse yaml")?;
        let doc = &docs[0];
        let root = self.tree.lookup_directory(&Path::root()).chain_err(|| "lookup root")?;
        ensure!(0 == root.list_directory().len(), "tree is not empty");
        Self::_seed_layer(root, Self::_hash_to_map(doc).chain_err(|| "hash to map")?).chain_err(|| "seed root layer")?;
        return Ok(());
    }

    fn _hash_to_map(yaml: &Yaml) -> Result<HashMap<String, &Yaml>> {
        ensure!(yaml.as_hash().is_some(), "encountered non-hash");
        let mut out = HashMap::new();
        for (k, v) in yaml.as_hash().unwrap() {
            ensure!(k.as_str().is_some(), "encounted non-string key");
            out.insert(k.as_str().unwrap().to_owned(), v);
        }
        return Ok(out);
    }

    fn _array_to_map(yaml: &Yaml) -> Result<HashMap<String, &Yaml>> {
        ensure!(yaml.as_vec().is_some(), "encountered non-array");
        let mut out = HashMap::new();
        for (e, v) in yaml.as_vec().unwrap().iter().enumerate() {
            out.insert(format!("{}", e), v);
        }
        return Ok(out);

    }

    fn _seed_layer(dir_node: &mut DirectoryData, map: HashMap<String, &Yaml>) -> Result<()> {
        for (name, value) in map {
            if let Some(formula) = Self::_build_formula(value).chain_err(|| "build formula")? {
                dir_node.graft_formula(&name, formula).chain_err(|| "graft formula")?;
            } else if value.as_hash().is_some() {
                let child_dir = dir_node.add_directory(&name).chain_err(|| "add directory")?;
                Self::_seed_layer(child_dir, Self::_hash_to_map(value).chain_err(|| "hash to map")?).chain_err(|| "seed child layer")?;
            } else if value.as_vec().is_some() {
                let child_dir = dir_node.add_directory(&name).chain_err(|| "add directory")?;
                Self::_seed_layer(child_dir, Self::_array_to_map(value).chain_err(|| "array to map")?).chain_err(|| "seed child layer")?;
            } else {
                let child_file = dir_node.add_file(&name).chain_err(|| "add file")?;
                let fmt = match value {
                    &Yaml::String(ref s) => s.to_owned(),
                    &Yaml::Real(ref s) => s.to_owned(),
                    &Yaml::Integer(i) => format!("{}", i),
                    &Yaml::Boolean(b) => format!("{}", b),
                    &Yaml::Null => "null".to_owned(),
                    &Yaml::BadValue => bail!("unexpected badvalue"),
                    &Yaml::Hash(_) => bail!("unexpected hash "),
                    &Yaml::Array(_) => bail!("unexpected array"),
                    &Yaml::Alias(_) => bail!("do not know how to handle alias value"),
                };
                child_file.set_data(&fmt);
            }
        }
        return Ok(());
    }

    fn _build_formula(map: &Yaml) -> Result<Option<FormulaData>> {
        if map["formula"].is_badvalue() || map["where"].is_badvalue() {
            return Ok(None);
        }
        ensure!(map["where"].as_hash().is_some(), "expected where to be a hash");
        let mut inputs: HashMap<String, Path> = HashMap::new();
        for (key, value) in map["where"].as_hash().unwrap().iter() {
            ensure!(key.as_str().is_some(), "expected string names in input");
            ensure!(value.as_str().is_some(), "expected a path as input source");
            let name = key.as_str().unwrap();
            let path = PathBuilder::new(value.as_str().unwrap()).chain_err(|| "new path builder")?.finish_path().chain_err(|| "finish path")?;
            inputs.insert(name.to_owned(), path);
        }
        ensure!(map["formula"].as_str().is_some(), "expected formula to be a string");
        return Ok(Some(FormulaData::new(&inputs, map["formula"].as_str().unwrap()).chain_err(|| "FormulaData new")?));
    }
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
            CallRequest::SeedDatabase(path) =>
                Response::SeedStatus(state.seed_tree(path))
        };
        return Ok((response, state));
    }
}

pub struct TreeServer {
    gen_server: GenServer<CastRequest, CallRequest, Response>
}

unsafe impl Send for TreeServer {}
unsafe impl Sync for TreeServer {}

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

    pub fn seed(&self, path: String) -> Result<()> {
        let response = self.gen_server.call(CallRequest::SeedDatabase(path)).chain_err(|| "call)")?;
        if let Response::SeedStatus(result) = response { return result; }
        bail!("wrong response type for seed");
    }
}

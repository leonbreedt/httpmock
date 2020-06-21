use crate::api::mock::Mock;
use crate::server::data::{MockServerState, ActiveMock, MockIdentification, MockDefinition};
use std::sync::Arc;
use std::net::{SocketAddr, ToSocketAddrs};
use hyper::{Request, StatusCode, Body, Error, Method as HyperMethod};
use std::fmt::Debug;
use std::cell::RefCell;
use hyper::body::Bytes;
use crate::server::{start_server, HttpMockConfig};

/// Refer to [regex::Regex](../regex/struct.Regex.html).
pub type Regex = regex::Regex;

thread_local!(
    static TOKIO_RUNTIME: RefCell<tokio::runtime::Runtime> = {
        let runtime = tokio::runtime::Builder::new()
            .enable_all()
            .basic_scheduler()
            .build()
            .expect("Cannot build thread local tokio tuntime");
        RefCell::new(runtime)
    };
);

/// This adapter allows to access the servers management functionality.
///
/// You can create an adapter by calling `ServerAdapter::from_env` to create a new instance.
/// You should never actually need to use this adapter, but you certainly can, if you absolutely
/// need to.
#[derive(Debug)]
pub struct MockServerHttpAdapter {
    pub(crate) host: String,
    pub(crate) port: u16,
}

impl MockServerHttpAdapter {
    pub(crate) fn new(host: String, port: u16) -> MockServerHttpAdapter {
        MockServerHttpAdapter { host, port }
    }

    pub fn server_port(&self) -> u16 {
        self.port
    }

    pub fn server_host(&self) -> &str {
        &self.host
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server_host(), self.server_port())
    }

    pub fn create_mock(&self, mock: &MockDefinition) -> Result<MockIdentification, String> {
        // Serialize to JSON
        let json = serde_json::to_string(mock);
        if let Err(err) = json {
            return Err(format!("cannot serialize mock object to JSON: {}", err));
        }
        let json = json.unwrap();

        // Send the request to the mock server
        let request_url = format!("http://{}/__mocks", &self.server_address());

        let request = Request::builder()
            .method(HyperMethod::POST)
            .uri(request_url)
            .header("Content-Type", "application/json")
            .body(Body::from(json))
            .expect("Cannot build request");

        let response = execute_request(request);
        if let Err(err) = response {
            return Err(format!("cannot send request to mock server: {}", err));
        }

        let (status, body) = response.unwrap();

        // Evaluate the response status
        if status != 201 {
            return Err(format!(
                "could not create mock. Mock server response: status = {}, message = {}",
                status, body
            ));
        }

        // Create response object
        let response: serde_json::Result<MockIdentification> = serde_json::from_str(&body);
        if let Err(err) = response {
            return Err(format!("cannot deserialize mock server response: {}", err));
        }

        return Ok(response.unwrap());
    }

    pub fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, String> {
        // Send the request to the mock server
        let request_url = format!("http://{}/__mocks/{}", &self.server_address(), mock_id);
        let request = Request::builder()
            .method(HyperMethod::GET)
            .uri(request_url)
            .body(Body::empty())
            .expect("Cannot build request");

        let response = execute_request(request);
        if let Err(err) = response {
            return Err(format!("cannot send request to mock server: {}", err));
        }

        let (status, body) = response.unwrap();

        // Evaluate response status code
        if status != 200 {
            return Err(format!(
                "could not create mock. Mock server response: status = {}, message = {}",
                status, body
            ));
        }

        // Create response object
        let response: serde_json::Result<ActiveMock> = serde_json::from_str(&body);
        if let Err(err) = response {
            return Err(format!("cannot deserialize mock server response: {}", err));
        }

        return Ok(response.unwrap());
    }

    pub fn delete_mock(&self, mock_id: usize) -> Result<(), String> {
        // Send the request to the mock server
        let request_url = format!("http://{}/__mocks/{}", &self.server_address(), mock_id);
        let request = Request::builder()
            .method(HyperMethod::DELETE)
            .uri(request_url)
            .body(Body::empty())
            .expect("Cannot build request");

        let response = execute_request(request);
        if let Err(err) = response {
            return Err(format!("cannot send request to mock server: {}", err));
        }
        let (status, body) = response.unwrap();

        // Evaluate response status code
        if status != 202 {
            return Err(format!(
                "Could not delete mocks from server (status = {}, message = {})",
                status, body
            ));
        }

        return Ok(());
    }

    pub fn delete_all_mocks(&self) -> Result<(), String> {
        // Send the request to the mock server
        let request_url = format!("http://{}/__mocks", &self.server_address());
        let request = Request::builder()
            .method(HyperMethod::DELETE)
            .uri(request_url)
            .body(Body::empty())
            .expect("Cannot build request");

        let response = execute_request(request);
        if let Err(err) = response {
            return Err(format!("cannot send request to mock server: {}", err));
        }

        let (status, body) = response.unwrap();

        // Evaluate response status code
        if status != 202 {
            return Err(format!(
                "Could not delete mocks from server (status = {}, message = {})",
                status, body
            ));
        }

        return Ok(());
    }
}

/// Represents an HTTP method.
#[derive(Debug)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

/// Enables enum to_string conversion
impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

/// Executes an HTTP request synchronously
fn execute_request(req: Request<Body>) -> Result<(StatusCode, String), Error> {
    return TOKIO_RUNTIME.with(|runtime| {
        let local = tokio::task::LocalSet::new();
        let mut rt = &mut *runtime.borrow_mut();
        return local.block_on(&mut rt, async {
            let client = hyper::Client::new();

            let resp = client.request(req).await.unwrap();
            let status = resp.status();

            let body: Bytes = hyper::body::to_bytes(resp.into_body()).await.unwrap();

            let body_str = String::from_utf8(body.to_vec()).unwrap();

            Ok((status, body_str))
        });
    });
}

pub struct LocalMockServerAdapter {
    shutdown_sender: Option<tokio::sync::oneshot::Sender<()>>,
    local_state: Arc<MockServerState>
}

impl LocalMockServerAdapter {
    pub(crate) fn new(  shutdown_sender: tokio::sync::oneshot::Sender<()>, local_state: Arc<MockServerState>) -> LocalMockServerAdapter {
        LocalMockServerAdapter { shutdown_sender: Some(shutdown_sender), local_state }
    }
}

impl Drop for LocalMockServerAdapter {
    fn drop(&mut self) {
        println!("IN DROP!");
        let shutdown_sender = std::mem::replace(&mut self.shutdown_sender, None);
        let shutdown_sender = shutdown_sender.expect("Cannot get shutdown sender.");
        if let Err(e) = shutdown_sender.send(()) {
            println!("Cannot send mock server shutdown signal.");
            log::warn!("Cannot send mock server shutdown signal: {:?}", e)
        }
    }
}

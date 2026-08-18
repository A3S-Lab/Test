use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const MAX_REQUEST_BYTES: usize = 16 * 1024;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(2);

const HERMETIC_HTML: &str = include_str!("../../../../fixtures/web/hermetic.html");
const ADVANCED_HTML: &str = include_str!("../../../../fixtures/web/advanced.html");
const LAYOUT_HTML: &str = include_str!("../../../../fixtures/web/layout.html");
const RENDERED_HTML: &str = include_str!("../../../../fixtures/web/rendered.html");
const TRANSIENT_HTML: &str = include_str!("../../../../fixtures/web/transient.html");
const TESTKIT_HTML: &str = include_str!("../../../../packages/testkit/src/browser-fixture.html");
const SCREEN_READER_WORKFLOWS: &str =
    include_str!("../../../../packages/testkit/screen-reader-audit/workflows.json");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
}

#[derive(Debug)]
pub struct TestHttpResponse {
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

pub struct WebFixture {
    primary: FixtureServer,
    blocked: FixtureServer,
}

impl WebFixture {
    pub fn start() -> io::Result<Self> {
        let blocked = FixtureServer::start(Site::Blocked)?;
        let primary = FixtureServer::start(Site::Primary {
            blocked_origin: blocked.sentinel_origin(),
        })?;
        Ok(Self { primary, blocked })
    }

    pub fn origin(&self) -> String {
        self.primary.origin()
    }

    pub fn blocked_origin(&self) -> String {
        self.blocked.sentinel_origin()
    }

    pub fn address(&self) -> SocketAddr {
        self.primary.address
    }

    pub fn primary_requests(&self) -> Vec<RecordedRequest> {
        self.primary.requests()
    }

    pub fn blocked_requests(&self) -> Vec<RecordedRequest> {
        self.blocked.requests()
    }

    pub fn clear_blocked_requests(&self) {
        self.blocked.clear_requests();
    }
}

pub fn assert_blocked_sentinel_reachable(fixture: &WebFixture) {
    let health = get(&fixture.blocked_origin(), "/health").expect("blocked sentinel health");
    assert_eq!(health.status, 200);
    assert_eq!(health.body, b"ready");
    assert_eq!(
        fixture.blocked_requests(),
        [RecordedRequest {
            method: "GET".to_string(),
            path: "/health".to_string(),
        }]
    );
    fixture.clear_blocked_requests();
}

pub fn start_testkit_fixture(bundle: Vec<u8>) -> io::Result<TestKitFixture> {
    let repaired = Arc::new(AtomicBool::new(false));
    FixtureServer::start(Site::TestKit {
        bundle,
        repaired: Arc::clone(&repaired),
    })
    .map(|server| TestKitFixture { server, repaired })
}

pub fn start_static_site_fixture(root: &Path, base: &str) -> io::Result<StaticSiteFixture> {
    if !base.starts_with('/')
        || !base.ends_with('/')
        || base.contains('\\')
        || base.split('/').any(|segment| segment == "..")
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "static fixture base must be an absolute, trailing-slash URL path",
        ));
    }
    let root = root.canonicalize()?;
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "static fixture root must be a directory",
        ));
    }
    FixtureServer::start(Site::Static {
        root,
        base: base.to_string(),
    })
    .map(|server| StaticSiteFixture { server })
}

pub struct TestKitFixture {
    server: FixtureServer,
    repaired: Arc<AtomicBool>,
}

pub struct StaticSiteFixture {
    server: FixtureServer,
}

impl StaticSiteFixture {
    pub fn origin(&self) -> String {
        self.server.origin()
    }

    pub fn address(&self) -> SocketAddr {
        self.server.address
    }
}

impl TestKitFixture {
    pub fn origin(&self) -> String {
        self.server.origin()
    }

    pub fn set_repaired(&self, repaired: bool) {
        self.repaired.store(repaired, Ordering::Release);
    }
}

pub fn get(origin: &str, path: &str) -> io::Result<TestHttpResponse> {
    let address = origin
        .strip_prefix("http://")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "expected an HTTP origin"))?;
    let mut stream = TcpStream::connect(address)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )?;
    stream.flush()?;
    stream.shutdown(Shutdown::Write)?;

    let response = read_response(&mut stream)?;
    parse_response(&response)
}

struct FixtureServer {
    address: SocketAddr,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}

impl FixtureServer {
    fn start(site: Site) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;

        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let worker_requests = Arc::clone(&requests);
        let worker_stop = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name(format!("a3s-web-fixture-{}", address.port()))
            .spawn(move || serve(listener, site, &worker_requests, &worker_stop))?;

        Ok(Self {
            address,
            requests,
            stop,
            worker: Some(worker),
        })
    }

    fn origin(&self) -> String {
        format!("http://{}", self.address)
    }

    fn sentinel_origin(&self) -> String {
        format!("http://localhost:{}", self.address.port())
    }

    fn requests(&self) -> Vec<RecordedRequest> {
        self.requests
            .lock()
            .expect("Web fixture request log must not be poisoned")
            .clone()
    }

    fn clear_requests(&self) {
        self.requests
            .lock()
            .expect("Web fixture request log must not be poisoned")
            .clear();
    }
}

impl Drop for FixtureServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_millis(250));
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .expect("Web fixture worker must not panic")
                .expect("Web fixture listener must remain available until drop");
        }
    }
}

#[derive(Clone)]
enum Site {
    Primary {
        blocked_origin: String,
    },
    Blocked,
    TestKit {
        bundle: Vec<u8>,
        repaired: Arc<AtomicBool>,
    },
    Static {
        root: PathBuf,
        base: String,
    },
}

fn serve(
    listener: TcpListener,
    site: Site,
    requests: &Mutex<Vec<RecordedRequest>>,
    stop: &AtomicBool,
) -> io::Result<()> {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
                let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
                if let Ok(request) = read_request(&mut stream) {
                    requests
                        .lock()
                        .expect("Web fixture request log must not be poisoned")
                        .push(request.clone());
                    let response = route(&site, &request);
                    let _ = write_response(&mut stream, &request.method, response);
                }
            }
            Err(error) if transient_accept_error(&error) => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn transient_accept_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::Interrupted
            | io::ErrorKind::WouldBlock
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

fn read_request(stream: &mut TcpStream) -> io::Result<RecordedRequest> {
    let mut bytes = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 1024];
    while bytes.len() < MAX_REQUEST_BYTES {
        let count = stream.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    if bytes.len() >= MAX_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "fixture request headers exceeded the size limit",
        ));
    }

    let request = std::str::from_utf8(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "request was not UTF-8"))?;
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request line was missing"))?;
    let mut fields = request_line.split_whitespace();
    let method = fields
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request method was missing"))?;
    let target = fields
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "request target was missing"))?;
    let version = fields
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "HTTP version was missing"))?;
    if fields.next().is_some() || !version.starts_with("HTTP/1.") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request line was malformed",
        ));
    }

    Ok(RecordedRequest {
        method: method.to_string(),
        path: target.split('?').next().unwrap_or(target).to_string(),
    })
}

fn route(site: &Site, request: &RecordedRequest) -> Response {
    if request.method != "GET" && request.method != "HEAD" {
        return Response::text("405 Method Not Allowed", "method not allowed")
            .with_header("Allow", "GET, HEAD");
    }

    match site {
        Site::Primary { blocked_origin } => route_primary(&request.path, blocked_origin),
        Site::Blocked => route_blocked(&request.path),
        Site::TestKit { bundle, repaired } => {
            route_testkit(&request.path, bundle, repaired.load(Ordering::Acquire))
        }
        Site::Static { root, base } => route_static(&request.path, root, base),
    }
}

fn route_static(path: &str, root: &Path, base: &str) -> Response {
    if path == "/health" {
        return Response::text("200 OK", "ready");
    }
    let Some(relative) = path.strip_prefix(base) else {
        return Response::text("404 Not Found", "not found");
    };
    if relative.contains('\\')
        || relative
            .split('/')
            .any(|segment| segment == "." || segment == "..")
    {
        return Response::text("404 Not Found", "not found");
    }

    let relative = if relative.is_empty() || relative.ends_with('/') {
        format!("{relative}index.html")
    } else {
        relative.to_string()
    };
    let candidate = root.join(&relative);
    let Ok(candidate) = candidate.canonicalize() else {
        return Response::text("404 Not Found", "not found");
    };
    if !candidate.starts_with(root) || !candidate.is_file() {
        return Response::text("404 Not Found", "not found");
    }
    let Ok(body) = std::fs::read(&candidate) else {
        return Response::text("500 Internal Server Error", "failed to read fixture asset");
    };
    Response::bytes(content_type(&candidate), body)
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("html") => "text/html; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("md") | Some("txt") => "text/plain; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn route_testkit(path: &str, bundle: &[u8], repaired: bool) -> Response {
    match path {
        "/" | "/testkit.html" => Response::html(
            TESTKIT_HTML
                .replace(
                    "__INITIAL_REPAIRED__",
                    if repaired { "true" } else { "false" },
                )
                .replace(
                    "__ACTION_LABEL__",
                    if repaired {
                        "Repaired action"
                    } else {
                        "Broken action"
                    },
                ),
        ),
        "/testkit.js" => Response::javascript_bytes(bundle.to_vec()),
        "/screen-reader-workflows.json" => Response::json(SCREEN_READER_WORKFLOWS.to_string()),
        "/health" => Response::text("200 OK", "ready"),
        _ => Response::text("404 Not Found", "not found"),
    }
}

fn route_primary(path: &str, blocked_origin: &str) -> Response {
    match path {
        "/" | "/index.html" => Response::html(
            HERMETIC_HTML.replace("__BLOCKED_ORIGIN__", blocked_origin),
        ),
        "/advanced.html" => Response::html(ADVANCED_HTML.to_string()),
        "/layout.html" => Response::html(LAYOUT_HTML.to_string()),
        "/rendered.html" => Response::html(RENDERED_HTML.to_string()),
        "/transient.html" => Response::html(TRANSIENT_HTML.to_string()),
        "/origin-policy.html" => Response::html(origin_policy_html(blocked_origin)),
        "/health" => Response::text("200 OK", "ready"),
        "/next" => Response::html(
            "<!doctype html><html lang=\"en\"><title>Next</title><body><h1>Same-origin navigation passed</h1></body></html>"
                .to_string(),
        ),
        "/redirect-same" => Response::redirect("/next"),
        "/redirect-cross" => Response::redirect(&format!("{blocked_origin}/blocked")),
        _ => Response::text("404 Not Found", "not found"),
    }
}

fn origin_policy_html(blocked_origin: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>A3S Test browser containment fixture</title>
  </head>
  <body>
    <h1>Browser containment fixture ready</h1>
    <a id="cross-origin" href="{blocked_origin}/blocked">
      Cross-origin link
    </a>
    <iframe title="Cross-origin redirect probe" src="/redirect-cross"></iframe>
    <script src="{blocked_origin}/blocked.js"></script>
    <img src="{blocked_origin}/blocked.png" alt="Cross-origin image probe" />
    <script>
      fetch("{blocked_origin}/blocked-fetch").catch(() => {{}});
    </script>
  </body>
</html>"#
    )
}

fn route_blocked(path: &str) -> Response {
    match path {
        "/blocked" => Response::html(
            "<!doctype html><html lang=\"en\"><title>Blocked</title><body><h1>Cross-origin request escaped</h1></body></html>"
                .to_string(),
        ),
        "/blocked.js" => Response::javascript(
            "document.documentElement.dataset.crossOriginScript = 'loaded';".to_string(),
        ),
        "/health" => Response::text("200 OK", "ready"),
        _ => Response::text("404 Not Found", "not found"),
    }
}

struct Response {
    status: &'static str,
    content_type: &'static str,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn bytes(content_type: &'static str, body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            content_type,
            headers: Vec::new(),
            body,
        }
    }

    fn html(body: String) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/html; charset=utf-8",
            headers: Vec::new(),
            body: body.into_bytes(),
        }
    }

    fn javascript(body: String) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/javascript; charset=utf-8",
            headers: Vec::new(),
            body: body.into_bytes(),
        }
    }

    fn javascript_bytes(body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            content_type: "text/javascript; charset=utf-8",
            headers: Vec::new(),
            body,
        }
    }

    fn json(body: String) -> Self {
        Self {
            status: "200 OK",
            content_type: "application/json; charset=utf-8",
            headers: Vec::new(),
            body: body.into_bytes(),
        }
    }

    fn text(status: &'static str, body: &'static str) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    fn redirect(location: &str) -> Self {
        Self {
            status: "302 Found",
            content_type: "text/plain; charset=utf-8",
            headers: vec![("Location", location.to_string())],
            body: b"redirecting".to_vec(),
        }
    }

    fn with_header(mut self, name: &'static str, value: &'static str) -> Self {
        self.headers.push((name, value.to_string()));
        self
    }
}

fn write_response(stream: &mut TcpStream, method: &str, response: Response) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n",
        response.status,
        response.content_type,
        response.body.len()
    )?;
    for (name, value) in response.headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;
    if method != "HEAD" {
        stream.write_all(&response.body)?;
    }
    stream.flush()?;
    stream.shutdown(Shutdown::Write)
}

fn read_response(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut response = Vec::with_capacity(1024);
    let mut buffer = [0_u8; 4096];
    loop {
        if let Some(expected_length) = framed_response_length(&response)? {
            if response.len() >= expected_length {
                response.truncate(expected_length);
                return Ok(response);
            }
        }
        if response.len() >= MAX_RESPONSE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "fixture response exceeded the size limit",
            ));
        }
        let remaining = MAX_RESPONSE_BYTES - response.len();
        let read_length = remaining.min(buffer.len());
        let count = stream.read(&mut buffer[..read_length])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "fixture response ended before its declared Content-Length",
            ));
        }
        response.extend_from_slice(&buffer[..count]);
    }
}

fn framed_response_length(response: &[u8]) -> io::Result<Option<usize>> {
    let Some(header_end) = response.windows(4).position(|window| window == b"\r\n\r\n") else {
        return Ok(None);
    };
    let headers = std::str::from_utf8(&response[..header_end]).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "response headers were not UTF-8",
        )
    })?;
    let content_length = headers
        .lines()
        .skip(1)
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then_some(value.trim())
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "fixture response omitted Content-Length",
            )
        })?
        .parse::<usize>()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "fixture response Content-Length was invalid",
            )
        })?;
    let total = header_end
        .checked_add(4)
        .and_then(|length| length.checked_add(content_length))
        .filter(|length| *length <= MAX_RESPONSE_BYTES)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "fixture response Content-Length exceeded the size limit",
            )
        })?;
    Ok(Some(total))
}

fn parse_response(response: &[u8]) -> io::Result<TestHttpResponse> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "response headers were missing")
        })?;
    let header_bytes = &response[..header_end];
    let header_text = std::str::from_utf8(header_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "response headers were not UTF-8",
        )
    })?;
    let mut lines = header_text.lines();
    let status_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "status line was missing"))?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "status code was invalid"))?;
    let mut headers = BTreeMap::new();
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "header was malformed"))?;
        headers.insert(name.to_ascii_lowercase(), value.trim().to_string());
    }
    Ok(TestHttpResponse {
        status,
        headers,
        body: response[header_end + 4..].to_vec(),
    })
}

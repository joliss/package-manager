use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use std::iter::FromIterator;
use std::sync::{Arc, Mutex};
use std::thread;

use rand::{Rng, OsRng};
use data_encoding::HEXUPPER;
use url::{Url, form_urlencoded};
use webbrowser;
use im::Map;

use futures::{Async, Poll};
use futures::task::{Task, current};
use futures::future::{self, Future, FutureResult};
use hyper::{self, StatusCode};
use hyper::server::{Http, Service, Request, Response};
use hyper::header::{ContentLength, ContentType};

use error::Error;
use config::{Config, Auth, get_config, write_config};

pub const USAGE: &'static str = "Login.

Usage:
    pm login [options]

Options:
    -h, --help     Display this message.
";

#[derive(Debug, Deserialize)]
pub struct Args {}



const AUTHENTICATED_DOC: &'static str = "
<html>
  <head>
    <style>
      body { text-align: center; background: white; }
    </style>
  </head>
  <body>
    <h1>You are authenticated!</h1>
    <p>You may safely close this window.</p>
  </body>
</html>
";



#[derive(Clone)]
struct Done<A> {
    value: Arc<Mutex<Option<A>>>,
    task: Arc<Mutex<Option<Task>>>,
}

impl<A> Done<A> {
    fn new() -> Self {
        Done {
            value: Arc::new(Mutex::new(None)),
            task: Arc::new(Mutex::new(None)),
        }
    }

    fn done(&self, value: A) {
        match self.value.lock() {
            Ok(ref mut mutex) => **mutex = Some(value),
            _ => panic!("failed to acquire mutex!?"),
        }
        match self.task.lock() {
            Ok(ref mutex) => {
                match **mutex {
                    Some(ref task) => {
                        task.notify();
                    }
                    None => (),
                }
            }
            _ => panic!("failed to acquire mutex!?"),
        }
    }
}

impl<A: Clone> Done<A> {
    fn get(&self) -> Option<A> {
        match self.value.lock() {
            Ok(ref mutex) => (**mutex).clone(),
            _ => panic!("failed to acquire mutex!?"),
        }
    }
}

impl<A> Future for Done<A> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.value.lock() {
            Ok(ref mutex) => {
                if (**mutex).is_some() {
                    Ok(Async::Ready(()))
                } else {
                    match self.task.lock() {
                        Ok(ref mut mutex) => **mutex = Some(current()),
                        _ => panic!("failed to acquire mutex in poll!!11"),
                    }
                    Ok(Async::NotReady)
                }
            }
            _ => panic!("failed to acquire mutex in poll!!11"),
        }
    }
}



fn bad_request() -> Response {
    Response::new()
        .with_status(StatusCode::BadRequest)
        .with_header(ContentLength("400 Bad Request".len() as u64))
        .with_header(ContentType::plaintext())
        .with_body("400 Bad Request")
}

struct CallbackArgs {
    state: String,
    token: String,
}

fn parse_callback_args(req: &Request) -> Option<CallbackArgs> {
    req.uri().query().and_then(|query| {
        let q = Map::from_iter(form_urlencoded::parse(query.as_bytes()).into_owned());
        match (q.get(&"state".to_string()), q.get(&"token".to_string())) {
            (Some(ref state), Some(ref token)) if q.len() == 2 => Some(CallbackArgs {
                state: state.to_string(),
                token: token.to_string(),
            }),
            _ => None,
        }
    })
}

struct Callback {
    state: String,
    done: Done<String>,
}

impl Service for Callback {
    type Request = Request;
    type Response = Response;
    type Error = hyper::Error;
    type Future = FutureResult<Response, hyper::Error>;

    fn call(&self, req: Request) -> Self::Future {
        if let Some(args) = parse_callback_args(&req) {
            if args.state != self.state {
                return future::ok(bad_request());
            }
            self.done.done(args.token);
            future::ok(
                Response::new()
                    .with_header(ContentLength(AUTHENTICATED_DOC.len() as u64))
                    .with_header(ContentType::html())
                    .with_body(AUTHENTICATED_DOC),
            )
        } else {
            future::ok(bad_request())
        }
    }
}

pub fn generate_secret() -> Result<String, Error> {
    let data: Vec<u8> = OsRng::new()?.gen_iter::<u8>().take(32).collect();
    Ok(HEXUPPER.encode(&data))
}

pub fn execute(_: Args) -> Result<(), Error> {
    let done = Done::new();
    let callback_done = done.clone();
    let secret = generate_secret()?;
    let mut url = Url::parse("http://localhost:8000/login_client").unwrap();
    url.query_pairs_mut().append_pair("token", &secret);
    let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
    let server = Http::new()
        .bind(&socket, move || {
            Ok(Callback {
                state: secret.clone(),
                done: callback_done.clone(),
            })
        })
        .expect("unable to launch local web server");

    url.query_pairs_mut().append_pair(
        "callback",
        &format!("http://{}", server.local_addr().unwrap()),
    );
    thread::spawn(move || webbrowser::open(url.as_str()));

    server.run_until(done.clone()).unwrap();

    let token = done.get().expect(
        "unable to get auth token from web server",
    );

    let config = get_config()?;
    write_config(&Config {
        auth: Auth {
            token: Some(token),
            ..config.auth
        },
        ..config
    })?;

    Ok(())
}

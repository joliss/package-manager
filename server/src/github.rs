use std::env;
use std::io::Read;

use serde::ser::Serialize;
use serde::de::DeserializeOwned;

use reqwest;
use reqwest::header::{Accept, Authorization, qitem};
use reqwest::mime::APPLICATION_JSON;

use error::{Res, Error};
use auth::{AuthSource, AuthProvider};
use user::{User, Org, UserRecord, OrgRecord};

pub static GITHUB_CLIENT_ID: &'static str = "a009958d6b555fa8c1f7";



#[derive(Serialize)]
struct OAuthResponse {
    client_id: String,
    client_secret: String,
    code: String,
}

#[derive(Deserialize, Debug)]
pub struct OAuthToken {
    pub access_token: String,
    pub scope: String,
    pub token_type: String,
}

#[derive(Deserialize, Debug)]
pub struct GithubUser {
    login: String,
    id: usize,
    avatar_url: String,
    // gravatar_id: String,
}

#[derive(Deserialize, Debug)]
pub struct GithubEmail {
    email: String,
    primary: bool,
}

#[derive(Deserialize, Debug)]
pub struct GithubOrg {
    login: String,
    id: usize,
    // url: String,
    // description: String,
    // avatar_url: String,
}



pub struct Github {
    http: reqwest::Client,
}

impl Github {
    pub fn new() -> Res<Self> {
        Ok(Github { http: reqwest::Client::new() })
    }

    #[allow(dead_code)]
    fn post<A, B>(&self, url: &str, token: &str, payload: &A) -> Res<B>
    where
        A: Serialize,
        B: DeserializeOwned,
    {
        Ok(self.http
            .post(&format!("https://api.github.com/{}", url))
            .header(Accept(vec![qitem(APPLICATION_JSON)]))
            .header(Authorization(format!("token {}", token)))
            .form(payload)
            .send()?
            .json()?)
    }

    fn get<B>(&self, url: &str, token: &str) -> Res<B>
    where
        B: DeserializeOwned,
    {
        Ok(self.http
            .get(&format!("https://api.github.com/{}", url))
            .header(Accept(vec![qitem(APPLICATION_JSON)]))
            .header(Authorization(format!("token {}", token)))
            .send()?
            .json()?)
    }

    #[allow(dead_code)]
    fn get_string(&self, url: &str, token: &str) -> Res<String> {
        let mut s = String::new();
        let mut res = self.http
            .get(&format!("https://api.github.com/{}", url))
            .header(Accept(vec![qitem(APPLICATION_JSON)]))
            .header(Authorization(format!("token {}", token)))
            .send()?;
        res.read_to_string(&mut s)?;
        Ok(s)
    }

    pub fn validate_callback(&self, code: &str) -> Res<OAuthToken> {
        Ok(self.http
            .post("https://github.com/login/oauth/access_token")
            .header(Accept(vec![qitem(APPLICATION_JSON)]))
            .form(&OAuthResponse {
                client_id: GITHUB_CLIENT_ID.to_string(),
                client_secret: env::var("GITHUB_SECRET")?,
                code: code.to_string(),
            })
            .send()?
            .json()?)
    }
}

impl AuthProvider for Github {
    fn user(&self, token: &str) -> Res<UserRecord> {
        let user: GithubUser = self.get("user", token)?;
        let emails: Vec<GithubEmail> = self.get("user/emails", token)?;
        let email = emails
            .iter()
            .find(|e| e.primary)
            .or(emails.iter().next())
            .ok_or(Error::UserHasNoEmail(format!(
                "{}:{} ({})",
                AuthSource::Github,
                user.id,
                user.login
            )))?;
        Ok(UserRecord::new(&User {
                provider: AuthSource::Github,
                id: format!("{}", user.id),
            }, &user.login, &email.email, &user.avatar_url))
    }

    fn orgs(&self, token: &str) -> Res<Box<Iterator<Item = OrgRecord>>> {
        let orgs: Vec<GithubOrg> = self.get("user/orgs", token)?;
        Ok(Box::new(orgs.into_iter().map(|org| {
            OrgRecord {
                id: Org {
                    provider: AuthSource::Github,
                    id: format!("{}", org.id),
                },
                name: org.login,
            }
        })))
    }
}

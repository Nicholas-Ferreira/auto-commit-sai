use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use log::{error, info};
use sys_locale::get_locale;
use question::{Answer, Question};
use rand::seq::SliceRandom;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use schemars::{
    gen::{SchemaGenerator, SchemaSettings},
    JsonSchema,
};
use serde_json::json;
use spinners::{Spinner, Spinners};
use std::{
    io::Write,
    process::{Command, Stdio},
    str,
};

#[derive(Parser)]
#[command(version)]
#[command(name = "Auto Commit")]
#[command(author = "Nicholas Ferreira fork Miguel Piedrafita")]
#[command(about = "Automagically generate commit messages.", long_about = None)]
struct Cli {
    #[clap(flatten)]
    verbose: Verbosity<InfoLevel>,

    #[arg(
        long = "dry-run",
        help = "Output the generated message, but don't create a commit."
    )]
    dry_run: bool,

    #[arg(
        short,
        long,
        help = "Edit the generated commit message before committing."
    )]
    review: bool,

    #[arg(short, long, help = "Don't ask for confirmation before committing.")]
    force: bool,
}

#[derive(Debug, serde::Deserialize, JsonSchema)]
struct Commit {
    /// The title of the commit.
    title: String,

    /// An exhaustive description of the changes.
    description: String,
}

impl ToString for Commit {
    fn to_string(&self) -> String {
        format!("{}\n\n{}", self.title, self.description)
    }
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .init();

    let api_token = std::env::var("SAI_API_KEY").unwrap_or_else(|_| {
        error!("Please set the SAI_API_KEY environment variable.");
        std::process::exit(1);
    });

    let git_staged_cmd = Command::new("git")
        .arg("diff")
        .arg("--staged")
        .output()
        .expect("Couldn't find diff.")
        .stdout;

    let git_staged_cmd = str::from_utf8(&git_staged_cmd).unwrap();

    if git_staged_cmd.is_empty() {
        error!("There are no staged files to commit.\nTry running `git add` to stage some files.");
        std::process::exit(1);
    }

    let is_repo = Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .expect("Failed to check if this is a git repository.")
        .stdout;

    if str::from_utf8(&is_repo).unwrap().trim() != "true" {
        error!("It looks like you are not in a git repository.\nPlease run this command from the root of a git repository, or initialize one using `git init`.");
        std::process::exit(1);
    }

    let client = Client::new();

    let output = Command::new("git")
        .arg("diff")
        .arg("HEAD")
        .output()
        .expect("Couldn't find diff.")
        .stdout;
    let output = str::from_utf8(&output).unwrap();

    if !cli.dry_run {
        info!("Loading Data...");
    }

    let sp: Option<Spinner> = if !cli.dry_run && cli.verbose.is_silent() {
        let vs = [
            Spinners::Earth,
            Spinners::Aesthetic,
            Spinners::Hearts,
            Spinners::BoxBounce,
            Spinners::BoxBounce2,
            Spinners::BouncingBar,
            Spinners::Christmas,
            Spinners::Clock,
            Spinners::FingerDance,
            Spinners::FistBump,
            Spinners::Flip,
            Spinners::Layer,
            Spinners::Line,
            Spinners::Material,
            Spinners::Mindblown,
            Spinners::Monkey,
            Spinners::Noise,
            Spinners::Point,
            Spinners::Pong,
            Spinners::Runner,
            Spinners::SoccerHeader,
            Spinners::Speaker,
            Spinners::SquareCorners,
            Spinners::Triangle,
        ];

        let spinner = vs.choose(&mut rand::thread_rng()).unwrap().clone();

        Some(Spinner::new(spinner, "Analyzing Codebase...".into()))
    } else {
        None
    };

    let mut headers = HeaderMap::new();
    headers.insert("X-Api-Key", HeaderValue::from_str(&api_token).unwrap());
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));

    let language = get_locale().unwrap_or_else(|| "pt-BR".to_string());

    let body = json!({
        "inputs": {
            "diff": output,
            "language": language,
        }
    });

    let response = client
        .post("https://sai-library.saiapplications.com/api/templates/66b12119075c349831386040/execute")
        .headers(headers)
        .json(&body)
        .send()
        .await
        .expect("Request failed");

    let commit_msg = response.text().await.expect("Couldn't parse response");
    
    if sp.is_some() {
        sp.unwrap().stop_with_message("Finished Analyzing!".into());
    }

    if cli.dry_run {
        info!("{}", commit_msg);
        return Ok(());
    } else {
        info!(
            "Proposed Commit:\n------------------------------\n{}\n------------------------------",
            commit_msg
        );

        if !cli.force {
            let answer = Question::new("Do you want to continue? (Y/n)")
                .yes_no()
                .until_acceptable()
                .default(Answer::YES)
                .ask()
                .expect("Couldn't ask question.");

            if answer == Answer::NO {
                error!("Commit aborted by user.");
                std::process::exit(1);
            }
            info!("Committing Message...");
        }
    }

    let mut ps_commit = Command::new("git")
        .arg("commit")
        .args(if cli.review { vec!["-e"] } else { vec![] })
        .arg("-F")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = ps_commit.stdin.take().expect("Failed to open stdin");
    std::thread::spawn(move || {
        stdin
            .write_all(commit_msg.as_bytes())
            .expect("Failed to write to stdin");
    });

    let commit_output = ps_commit
        .wait_with_output()
        .expect("There was an error when creating the commit.");

    info!("{}", str::from_utf8(&commit_output.stdout).unwrap());

    Ok(())
}

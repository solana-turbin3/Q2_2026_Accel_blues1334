use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use persistent_todo_queue::{next_id, now_unix, Queue, Store, Todo};

#[derive(Parser)]
#[command(name = "todo", about = "Persistent FIFO todo queue (Borsh-backed)")]
struct Cli {
    #[arg(long, default_value = "todos.bin", global = true)]
    file: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a task to the back of the queue.
    Add { description: String },
    /// List pending tasks in FIFO order.
    List,
    /// Remove and mark the oldest task as completed.
    Done,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> std::io::Result<()> {
    let store = Store::new(&cli.file);
    let mut queue: Queue<Todo> = store.load()?;

    match cli.command {
        Command::Add { description } => {
            let todo = Todo {
                id: next_id(&queue),
                description,
                created_at: now_unix(),
            };
            println!("added #{} \"{}\"", todo.id, todo.description);
            queue.enqueue(todo);
            store.save(&queue)?;
        }
        Command::List => {
            if queue.is_empty() {
                println!("(no pending tasks)");
            } else {
                for todo in queue.iter() {
                    println!("{:?}", todo);
                }
            }
        }
        Command::Done => match queue.dequeue() {
            Some(todo) => {
                println!("completed #{} \"{}\"", todo.id, todo.description);
                store.save(&queue)?;
            }
            None => println!("(no tasks to complete)"),
        },
    }
    Ok(())
}

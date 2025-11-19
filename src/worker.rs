//! Web crawler and parser.

use async_channel::Receiver;

pub type WorkerInbox = Receiver<WorkItem>;

pub async fn spawn_worker(inbox: WorkerInbox) {}

pub struct WorkItem;
